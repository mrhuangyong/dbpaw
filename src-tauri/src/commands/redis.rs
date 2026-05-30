use crate::datasources::redis::{
    self, RedisBatchKeyOp, RedisBatchKeyOpResult, RedisClusterInfo, RedisDatabaseInfo,
    RedisGeoMember, RedisGeoPosition, RedisGeoSearchResult, RedisKeyPatchPayload, RedisKeyValue,
    RedisLInsertPosition, RedisLMoveDirection, RedisMgetEntry, RedisMutationResult, RedisRawResult,
    RedisScanResponse, RedisServerInfo, RedisSetKeyPayload, RedisSetOperation, RedisSlowlogEntry,
    RedisStreamEntry, RedisStreamView, RedisXClaimEntry, RedisXPendingResult,
    RedisZRangeByLexResult, RedisZRangeByScoreResult, RedisZSetMember,
};
use crate::datasources::redis::{connect, RedisConnection};
use crate::models::{ConnectionForm, RedisCommandLog};
use crate::state::AppState;
use std::collections::HashMap;
use tauri::State;

async fn connection_form(state: &State<'_, AppState>, id: i64) -> Result<ConnectionForm, String> {
    let local_db = {
        let lock = state.local_db.lock().await;
        lock.clone()
    };
    let db = local_db.ok_or("Local DB not initialized")?;
    let form = db.get_connection_form_by_id(id).await?;
    if form.driver != "redis" {
        return Err(format!(
            "[UNSUPPORTED] Connection {} is not a Redis connection",
            id
        ));
    }
    Ok(form)
}

/// Cache key: standalone uses "{id}:{db}" so different databases on the same
/// server each get their own persistent connection (SELECT is connection-level).
/// Cluster uses "{id}:cluster" since it only supports db0.
fn cache_key(id: i64, database: Option<&str>, is_cluster: bool) -> String {
    if is_cluster {
        format!("{id}:cluster")
    } else {
        format!("{id}:{}", database.unwrap_or(""))
    }
}

/// Returns true if the error string looks like a broken/dropped TCP connection.
fn is_io_error(e: &str) -> bool {
    e.contains("[REDIS_ERROR]") && {
        let lower = e.to_lowercase();
        lower.contains("broken pipe")
            || lower.contains("connection reset")
            || lower.contains("connection refused")
            || lower.contains("connection closed")
            || lower.contains("eof")
            || lower.contains("os error")
    }
}

/// Get a cached connection for (id, database), creating one if not present.
async fn acquire(
    state: &State<'_, AppState>,
    id: i64,
    form: &ConnectionForm,
    database: Option<&str>,
) -> Result<RedisConnection, String> {
    let is_cluster = form
        .host
        .as_deref()
        .map(|h| h.split(',').filter(|p| !p.trim().is_empty()).count() > 1)
        .unwrap_or(false);
    let key = cache_key(id, database, is_cluster);

    // Fast path: return a clone of the cached connection
    {
        let cache = state.redis_cache.lock().await;
        if let Some(conn) = cache.get(&key) {
            return Ok(conn);
        }
    }

    // Slow path: create a new connection and cache it
    let conn = connect(form, database).await?;
    {
        let mut cache = state.redis_cache.lock().await;
        // Another task might have raced in; prefer the one already in the cache
        if let Some(existing) = cache.get(&key) {
            return Ok(existing);
        }
        cache.insert(key, conn.clone());
    }
    Ok(conn)
}

/// Remove a stale connection from the cache (called after an IO error).
async fn evict(
    state: &State<'_, AppState>,
    id: i64,
    form: &ConnectionForm,
    database: Option<&str>,
) {
    let is_cluster = form
        .host
        .as_deref()
        .map(|h| h.split(',').filter(|p| !p.trim().is_empty()).count() > 1)
        .unwrap_or(false);
    let key = cache_key(id, database, is_cluster);
    let mut cache = state.redis_cache.lock().await;
    cache.remove(&key);
}

#[tauri::command]
pub async fn redis_list_databases(
    state: State<'_, AppState>,
    id: i64,
) -> Result<Vec<RedisDatabaseInfo>, String> {
    let form = connection_form(&state, id).await?;
    let mut conn = acquire(&state, id, &form, None).await?;
    if let Err(e) = redis::ping(&mut conn).await {
        if is_io_error(&e) {
            evict(&state, id, &form, None).await;
        }
        return Err(e);
    }

    let db_count = if conn.is_cluster() {
        1
    } else {
        let mut cmd = ::redis::cmd("CONFIG");
        cmd.arg("GET").arg("databases");
        match conn.query::<Vec<String>>(cmd).await {
            Ok(values) if values.len() >= 2 => values[1].parse::<i64>().unwrap_or(16).clamp(1, 256),
            _ => 16,
        }
    };

    let mut dbs = redis::list_databases(&form, db_count)?;

    if conn.is_cluster() {
        if let Some(db) = dbs.first_mut() {
            let count: u64 = conn.query(::redis::cmd("DBSIZE")).await.unwrap_or(0);
            db.key_count = Some(count);
        }
    } else {
        for db in &mut dbs {
            let mut select_cmd = ::redis::cmd("SELECT");
            select_cmd.arg(db.index);
            let _ = conn.query::<()>(select_cmd).await;
            let count: u64 = conn.query(::redis::cmd("DBSIZE")).await.unwrap_or(0);
            db.key_count = Some(count);
        }
        // Restore to the originally selected database
        if let Some(selected) = dbs.iter().find(|d| d.selected) {
            let mut select_cmd = ::redis::cmd("SELECT");
            select_cmd.arg(selected.index);
            let _ = conn.query::<()>(select_cmd).await;
        }
    }

    Ok(dbs)
}

#[tauri::command]
pub async fn redis_scan_keys(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    cursor: Option<String>,
    pattern: Option<String>,
    limit: Option<u32>,
) -> Result<RedisScanResponse, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::scan_keys(&mut conn, cursor.clone(), pattern.clone(), limit).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::scan_keys(&mut conn, cursor, pattern, limit).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_get_key(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
) -> Result<RedisKeyValue, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::get_key(&mut conn, key.clone()).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::get_key(&mut conn, key).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_set_key(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    payload: RedisSetKeyPayload,
) -> Result<RedisMutationResult, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::set_key(&mut conn, payload.clone()).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::set_key(&mut conn, payload).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_update_key(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    payload: RedisSetKeyPayload,
) -> Result<RedisMutationResult, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::set_key(&mut conn, payload.clone()).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::set_key(&mut conn, payload).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_delete_key(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
) -> Result<RedisMutationResult, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::delete_key(&mut conn, key.clone()).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::delete_key(&mut conn, key).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_patch_key(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    payload: RedisKeyPatchPayload,
) -> Result<RedisMutationResult, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::patch_key(&mut conn, payload.clone()).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::patch_key(&mut conn, payload).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_rename_key(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    old_key: String,
    new_key: String,
    force: Option<bool>,
) -> Result<RedisMutationResult, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let force = force.unwrap_or(false);
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::rename_key(&mut conn, old_key.clone(), new_key.clone(), force).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::rename_key(&mut conn, old_key, new_key, force).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_get_key_page(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    offset: u64,
    limit: u32,
) -> Result<RedisKeyValue, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::get_key_page(&mut conn, key.clone(), offset, limit).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::get_key_page(&mut conn, key, offset, limit).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_set_ttl(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    ttl_seconds: Option<i64>,
) -> Result<RedisMutationResult, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::set_ttl(&mut conn, key.clone(), ttl_seconds).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::set_ttl(&mut conn, key, ttl_seconds).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_get_stream_range(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    start_id: String,
    count: u32,
) -> Result<Vec<RedisStreamEntry>, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::get_stream_range(&mut conn, key.clone(), start_id.clone(), count).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::get_stream_range(&mut conn, key, start_id, count).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_get_stream_view(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    start_id: String,
    end_id: String,
    count: u32,
) -> Result<RedisStreamView, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::get_stream_view(
        &mut conn,
        key.clone(),
        start_id.clone(),
        end_id.clone(),
        count,
    )
    .await
    {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::get_stream_view(&mut conn, key, start_id, end_id, count).await
        }
        r => r,
    }
}

async fn append_redis_command_log(
    state: &AppState,
    command: String,
    connection_id: i64,
    database: Option<String>,
    success: bool,
    error: Option<String>,
) {
    let db = {
        let lock = state.local_db.lock().await;
        lock.clone()
    };

    if let Some(local_db) = db {
        if let Err(e) = local_db
            .insert_redis_command_log(command, Some(connection_id), database, success, error)
            .await
        {
            eprintln!("[REDIS_LOG_APPEND_ERROR] {}", e);
        }
    }
}

#[tauri::command]
pub async fn redis_execute_raw(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    command: String,
) -> Result<RedisRawResult, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    let result = match redis::execute_raw(&mut conn, command.clone()).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::execute_raw(&mut conn, command.clone()).await
        }
        r => r,
    };

    match &result {
        Ok(_) => {
            append_redis_command_log(&state, command, id, database, true, None).await;
        }
        Err(e) => {
            append_redis_command_log(&state, command, id, database, false, Some(e.clone())).await;
        }
    }

    result
}

#[tauri::command]
pub async fn redis_bitmap_get_bit(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    offset: u64,
) -> Result<bool, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::bitmap_get_bit(&mut conn, key.clone(), offset).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::bitmap_get_bit(&mut conn, key, offset).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_bitmap_count(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    start: Option<i64>,
    end: Option<i64>,
) -> Result<u64, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::bitmap_count(&mut conn, key.clone(), start, end).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::bitmap_count(&mut conn, key, start, end).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_bitmap_pos(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    bit: bool,
    start: Option<u64>,
    end: Option<u64>,
    count: Option<u64>,
) -> Result<Vec<u64>, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::bitmap_pos(&mut conn, key.clone(), bit, start, end, count).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::bitmap_pos(&mut conn, key, bit, start, end, count).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_hll_pfadd(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    elements: Vec<String>,
) -> Result<bool, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::hll_pfadd(&mut conn, key.clone(), elements.clone()).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::hll_pfadd(&mut conn, key, elements).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_geo_add(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    members: Vec<RedisGeoMember>,
) -> Result<i64, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::geo_add(&mut conn, key.clone(), members.clone()).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::geo_add(&mut conn, key, members).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_geo_pos(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    members: Vec<String>,
) -> Result<Vec<Option<RedisGeoPosition>>, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::geo_pos(&mut conn, key.clone(), members.clone()).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::geo_pos(&mut conn, key, members).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_geo_dist(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    member1: String,
    member2: String,
    unit: Option<String>,
) -> Result<f64, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::geo_dist(
        &mut conn,
        key.clone(),
        member1.clone(),
        member2.clone(),
        unit.clone(),
    )
    .await
    {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::geo_dist(&mut conn, key, member1, member2, unit).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_geo_search(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    member: Option<String>,
    longitude: Option<f64>,
    latitude: Option<f64>,
    radius: f64,
    unit: String,
    with_coord: bool,
    with_dist: bool,
    with_hash: bool,
    count: Option<u64>,
) -> Result<Vec<RedisGeoSearchResult>, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::geo_search(
        &mut conn,
        key.clone(),
        member.clone(),
        longitude,
        latitude,
        radius,
        unit.clone(),
        with_coord,
        with_dist,
        with_hash,
        count,
    )
    .await
    {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::geo_search(
                &mut conn, key, member, longitude, latitude, radius, unit, with_coord, with_dist,
                with_hash, count,
            )
            .await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_server_info(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
) -> Result<RedisServerInfo, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::server_info(&mut conn).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::server_info(&mut conn).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_server_config(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
) -> Result<HashMap<String, String>, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::server_config(&mut conn).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::server_config(&mut conn).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_slowlog_get(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    count: Option<i64>,
) -> Result<Vec<RedisSlowlogEntry>, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    let n = count.unwrap_or(50);
    match redis::slowlog_get(&mut conn, n).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::slowlog_get(&mut conn, n).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_zrangebyscore(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    min: String,
    max: String,
    offset: Option<u64>,
    limit: Option<u64>,
) -> Result<RedisZRangeByScoreResult, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::zrangebyscore(
        &mut conn,
        key.clone(),
        min.clone(),
        max.clone(),
        offset,
        limit,
    )
    .await
    {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::zrangebyscore(&mut conn, key, min, max, offset, limit).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_zrank(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    member: String,
    reverse: Option<bool>,
) -> Result<Option<i64>, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    let rev = reverse.unwrap_or(false);
    match redis::zrank(&mut conn, key.clone(), member.clone(), rev).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::zrank(&mut conn, key, member, rev).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_set_operation(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    keys: Vec<String>,
    op: RedisSetOperation,
) -> Result<Vec<String>, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::set_operation(&mut conn, keys.clone(), op.clone()).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::set_operation(&mut conn, keys, op).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_sismember(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    member: String,
) -> Result<bool, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::sismember(&mut conn, key.clone(), member.clone()).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::sismember(&mut conn, key, member).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_smove(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    source: String,
    destination: String,
    member: String,
) -> Result<bool, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::smove(
        &mut conn,
        source.clone(),
        destination.clone(),
        member.clone(),
    )
    .await
    {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::smove(&mut conn, source, destination, member).await
        }
        r => r,
    }
}

// ── Stream Consumer Group commands ──────────────────────────────────────────

#[tauri::command]
pub async fn redis_xgroup_create(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    group: String,
    start_id: String,
    mkstream: Option<bool>,
) -> Result<bool, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    let ms = mkstream.unwrap_or(false);
    match redis::xgroup_create(&mut conn, key.clone(), group.clone(), start_id.clone(), ms).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::xgroup_create(&mut conn, key, group, start_id, ms).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_xgroup_del(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    group: String,
) -> Result<bool, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::xgroup_del(&mut conn, key.clone(), group.clone()).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::xgroup_del(&mut conn, key, group).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_xgroup_setid(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    group: String,
    start_id: String,
) -> Result<bool, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::xgroup_setid(&mut conn, key.clone(), group.clone(), start_id.clone()).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::xgroup_setid(&mut conn, key, group, start_id).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_xack(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    group: String,
    ids: Vec<String>,
) -> Result<i64, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::xack(&mut conn, key.clone(), group.clone(), ids.clone()).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::xack(&mut conn, key, group, ids).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_xpending(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    group: String,
    start: Option<String>,
    end: Option<String>,
    count: Option<i64>,
    consumer: Option<String>,
) -> Result<RedisXPendingResult, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::xpending(
        &mut conn,
        key.clone(),
        group.clone(),
        start.clone(),
        end.clone(),
        count,
        consumer.clone(),
    )
    .await
    {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::xpending(&mut conn, key, group, start, end, count, consumer).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_xclaim(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    group: String,
    consumer: String,
    min_idle_ms: i64,
    ids: Vec<String>,
) -> Result<Vec<RedisXClaimEntry>, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::xclaim(
        &mut conn,
        key.clone(),
        group.clone(),
        consumer.clone(),
        min_idle_ms,
        ids.clone(),
    )
    .await
    {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::xclaim(&mut conn, key, group, consumer, min_idle_ms, ids).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_xtrim(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    strategy: String,
    threshold: String,
    approximate: Option<bool>,
) -> Result<i64, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::xtrim(
        &mut conn,
        key.clone(),
        strategy.clone(),
        threshold.clone(),
        approximate,
    )
    .await
    {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::xtrim(&mut conn, key, strategy, threshold, approximate).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_xreadgroup(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    group: String,
    consumer: String,
    start_id: String,
    count: Option<i64>,
) -> Result<Vec<RedisStreamEntry>, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::xreadgroup(
        &mut conn,
        key.clone(),
        group.clone(),
        consumer.clone(),
        start_id.clone(),
        count,
    )
    .await
    {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::xreadgroup(&mut conn, key, group, consumer, start_id, count).await
        }
        r => r,
    }
}

// ── Batch operations ────────────────────────────────────────────────────────

#[tauri::command]
pub async fn redis_batch_key_ops(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    operations: Vec<RedisBatchKeyOp>,
) -> Result<Vec<RedisBatchKeyOpResult>, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::batch_key_ops(&mut conn, operations.clone()).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::batch_key_ops(&mut conn, operations).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_mget(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    keys: Vec<String>,
) -> Result<Vec<RedisMgetEntry>, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::mget_keys(&mut conn, keys.clone()).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::mget_keys(&mut conn, keys).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_mset(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    entries: HashMap<String, String>,
) -> Result<RedisMutationResult, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let pairs: Vec<(String, String)> = entries.into_iter().collect();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::mset_keys(&mut conn, pairs.clone()).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::mset_keys(&mut conn, pairs).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_cluster_info(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
) -> Result<RedisClusterInfo, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::cluster_info(&mut conn).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::cluster_info(&mut conn).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_zscore(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    member: String,
) -> Result<Option<f64>, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::zscore(&mut conn, key.clone(), member.clone()).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::zscore(&mut conn, key, member).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_zmscore(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    members: Vec<String>,
) -> Result<Vec<Option<f64>>, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::zmscore(&mut conn, key.clone(), members.clone()).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::zmscore(&mut conn, key, members).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_zrangebylex(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    min: String,
    max: String,
    offset: Option<u64>,
    limit: Option<u64>,
) -> Result<RedisZRangeByLexResult, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::zrangebylex(
        &mut conn,
        key.clone(),
        min.clone(),
        max.clone(),
        offset,
        limit,
    )
    .await
    {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::zrangebylex(&mut conn, key, min, max, offset, limit).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_zlexcount(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    min: String,
    max: String,
) -> Result<u64, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::zlexcount(&mut conn, key.clone(), min.clone(), max.clone()).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::zlexcount(&mut conn, key, min, max).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_zpopmin(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    count: Option<u64>,
) -> Result<Vec<RedisZSetMember>, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::zpopmin(&mut conn, key.clone(), count).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::zpopmin(&mut conn, key, count).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_zpopmax(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    count: Option<u64>,
) -> Result<Vec<RedisZSetMember>, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::zpopmax(&mut conn, key.clone(), count).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::zpopmax(&mut conn, key, count).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_lindex(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    index: i64,
) -> Result<Option<String>, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::lindex(&mut conn, key.clone(), index).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::lindex(&mut conn, key, index).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_lpos(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    element: String,
    rank: Option<i64>,
    count: Option<u64>,
    maxlen: Option<u64>,
) -> Result<Vec<i64>, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::lpos(&mut conn, key.clone(), element.clone(), rank, count, maxlen).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::lpos(&mut conn, key, element, rank, count, maxlen).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_ltrim(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    start: i64,
    stop: i64,
) -> Result<bool, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::ltrim(&mut conn, key.clone(), start, stop).await {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::ltrim(&mut conn, key, start, stop).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_linsert(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    key: String,
    position: RedisLInsertPosition,
    pivot: String,
    element: String,
) -> Result<i64, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::linsert(
        &mut conn,
        key.clone(),
        position.clone(),
        pivot.clone(),
        element.clone(),
    )
    .await
    {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::linsert(&mut conn, key, position, pivot, element).await
        }
        r => r,
    }
}

#[tauri::command]
pub async fn redis_lmove(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    source: String,
    destination: String,
    src_direction: RedisLMoveDirection,
    dst_direction: RedisLMoveDirection,
) -> Result<Option<String>, String> {
    let form = connection_form(&state, id).await?;
    let db = database.as_deref();
    let mut conn = acquire(&state, id, &form, db).await?;
    match redis::lmove(
        &mut conn,
        source.clone(),
        destination.clone(),
        src_direction.clone(),
        dst_direction.clone(),
    )
    .await
    {
        Err(ref e) if is_io_error(e) => {
            evict(&state, id, &form, db).await;
            let mut conn = acquire(&state, id, &form, db).await?;
            redis::lmove(&mut conn, source, destination, src_direction, dst_direction).await
        }
        r => r,
    }
}

fn clamp_redis_command_logs_limit(limit: Option<i64>) -> i64 {
    limit.unwrap_or(100).clamp(1, 100)
}

#[tauri::command]
pub async fn list_redis_command_logs(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<RedisCommandLog>, String> {
    let safe_limit = clamp_redis_command_logs_limit(limit);
    let local_db = {
        let lock = state.local_db.lock().await;
        lock.clone()
    };

    if let Some(db) = local_db {
        db.list_redis_command_logs(safe_limit).await
    } else {
        Err("Local DB not initialized".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_standalone_with_database() {
        assert_eq!(cache_key(1, Some("db0"), false), "1:db0");
    }

    #[test]
    fn cache_key_standalone_no_database() {
        assert_eq!(cache_key(1, None, false), "1:");
    }

    #[test]
    fn cache_key_cluster_with_database() {
        assert_eq!(cache_key(42, Some("db1"), true), "42:cluster");
    }

    #[test]
    fn cache_key_cluster_no_database() {
        assert_eq!(cache_key(42, None, true), "42:cluster");
    }

    #[test]
    fn cache_key_standalone_custom_db() {
        assert_eq!(cache_key(99, Some("mydb"), false), "99:mydb");
    }

    #[test]
    fn io_error_broken_pipe() {
        assert!(is_io_error("[REDIS_ERROR] broken pipe"));
    }

    #[test]
    fn io_error_connection_reset() {
        assert!(is_io_error("[REDIS_ERROR] connection reset by peer"));
    }

    #[test]
    fn io_error_connection_refused() {
        assert!(is_io_error("[REDIS_ERROR] connection refused"));
    }

    #[test]
    fn io_error_not_redis_error() {
        assert!(!is_io_error("some other error"));
    }

    #[test]
    fn io_error_redis_but_not_io() {
        assert!(!is_io_error("[REDIS_ERROR] ERR wrong number of arguments"));
    }

    #[test]
    fn clamp_none_returns_default() {
        assert_eq!(clamp_redis_command_logs_limit(None), 100);
    }

    #[test]
    fn clamp_within_range() {
        assert_eq!(clamp_redis_command_logs_limit(Some(50)), 50);
    }

    #[test]
    fn clamp_below_minimum() {
        assert_eq!(clamp_redis_command_logs_limit(Some(0)), 1);
    }

    #[test]
    fn clamp_above_maximum() {
        assert_eq!(clamp_redis_command_logs_limit(Some(200)), 100);
    }
}
