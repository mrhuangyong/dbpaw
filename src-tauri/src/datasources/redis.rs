use crate::db::drivers::conn_failed_error;
use crate::models::ConnectionForm;
use base64::Engine;
use redis::aio::ConnectionLike;
use redis::aio::MultiplexedConnection;
use redis::cluster::ClusterClient;
use redis::cluster_async::ClusterConnection;
use redis::cluster_routing::{
    MultipleNodeRoutingInfo, ResponsePolicy, RoutingInfo, SingleNodeRoutingInfo,
};
use redis::sentinel::{Sentinel, SentinelNodeConnectionInfo};
use redis::AsyncConnectionConfig;
use redis::{
    from_redis_value, Cmd, ConnectionAddr, ConnectionInfo, FromRedisValue, ProtocolVersion,
    RedisConnectionInfo, TlsMode, Value,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex as TokioMutex;

const DEFAULT_REDIS_PORT: i64 = 6379;
const DEFAULT_CONNECT_TIMEOUT_MS: i64 = 5000;
const DEFAULT_SCAN_LIMIT: u32 = 100;
const MAX_SCAN_LIMIT: u32 = 1000;
const PAGE_SIZE: isize = 200;

/// Shareable Redis connection handle.
/// Standalone uses MultiplexedConnection (Clone, shared underlying TCP).
/// Cluster wraps ClusterConnection in Arc<Mutex> so it can be shared across commands.
#[derive(Clone)]
pub enum RedisConnection {
    Standalone(MultiplexedConnection),
    Cluster(Arc<TokioMutex<ClusterConnection>>),
}

pub struct RedisConnectionCache {
    connections: HashMap<String, RedisConnection>,
}

impl RedisConnectionCache {
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
        }
    }

    pub fn get(&self, key: &str) -> Option<RedisConnection> {
        self.connections.get(key).cloned()
    }

    pub fn insert(&mut self, key: String, conn: RedisConnection) {
        self.connections.insert(key, conn);
    }

    pub fn remove(&mut self, key: &str) {
        self.connections.remove(key);
    }

    /// Remove all cached connections that belong to `connection_id`
    /// (keys are formatted as `"{id}:{db}"` or `"{id}:cluster"`).
    pub fn remove_by_connection_id(&mut self, connection_id: i64) {
        let prefix = format!("{connection_id}:");
        self.connections.retain(|k, _| !k.starts_with(&prefix));
    }
}

impl RedisConnection {
    pub fn is_cluster(&self) -> bool {
        matches!(self, RedisConnection::Cluster(_))
    }

    pub async fn query<T: FromRedisValue>(&mut self, cmd: Cmd) -> Result<T, String> {
        match self {
            RedisConnection::Standalone(inner) => query_on(inner, cmd).await,
            RedisConnection::Cluster(arc) => {
                let mut conn = arc.lock().await;
                query_on(&mut *conn, cmd).await
            }
        }
    }

    pub async fn route_all_masters_combine_arrays<T: FromRedisValue>(
        &mut self,
        cmd: &Cmd,
    ) -> Result<T, String> {
        let RedisConnection::Cluster(arc) = self else {
            return Err("[REDIS_ERROR] all-master routing requires Redis Cluster".to_string());
        };
        let mut cluster = arc.lock().await;
        let value = cluster
            .route_command(
                cmd,
                RoutingInfo::MultiNode((
                    MultipleNodeRoutingInfo::AllMasters,
                    Some(ResponsePolicy::CombineArrays),
                )),
            )
            .await
            .map_err(|e| format!("[REDIS_ERROR] {e}"))?;
        from_redis_value(&value).map_err(|e| format!("[REDIS_ERROR] {e}"))
    }

    pub async fn pipe_query<T: FromRedisValue>(
        &mut self,
        pipe: &mut redis::Pipeline,
    ) -> Result<T, String> {
        match self {
            RedisConnection::Standalone(inner) => pipe
                .query_async(inner)
                .await
                .map_err(|e| format!("[REDIS_ERROR] {e}")),
            RedisConnection::Cluster(arc) => {
                let mut conn = arc.lock().await;
                pipe.query_async(&mut *conn)
                    .await
                    .map_err(|e| format!("[REDIS_ERROR] {e}"))
            }
        }
    }

    pub async fn query_on_node<T: FromRedisValue>(
        &mut self,
        host: &str,
        port: u16,
        cmd: Cmd,
    ) -> Result<T, String> {
        let RedisConnection::Cluster(arc) = self else {
            return self.query(cmd).await;
        };
        let mut cluster = arc.lock().await;
        let routing = RoutingInfo::SingleNode(SingleNodeRoutingInfo::ByAddress {
            host: host.to_string(),
            port,
        });
        let value = cluster
            .route_command(&cmd, routing)
            .await
            .map_err(|e| format!("[REDIS_ERROR] {e}"))?;
        from_redis_value(&value).map_err(|e| format!("[REDIS_ERROR] {e}"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisDatabaseInfo {
    pub index: i64,
    pub name: String,
    pub selected: bool,
    pub key_count: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisServerInfo {
    pub sections: HashMap<String, HashMap<String, String>>,
    pub dbsize: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisSlowlogEntry {
    pub id: u64,
    pub timestamp: i64,
    pub duration_ms: u64,
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisKeyInfo {
    pub key: String,
    pub key_type: String,
    pub ttl: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisScanResponse {
    pub cursor: String,
    pub keys: Vec<RedisKeyInfo>,
    pub is_partial: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisZSetMember {
    pub member: String,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisStreamEntry {
    pub id: String,
    pub fields: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisStreamInfo {
    pub length: u64,
    pub radix_tree_keys: u64,
    pub radix_tree_nodes: u64,
    pub groups: u64,
    pub last_generated_id: String,
    pub first_entry: Option<RedisStreamEntry>,
    pub last_entry: Option<RedisStreamEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisStreamGroupInfo {
    pub name: String,
    pub consumers: u64,
    pub pending: u64,
    pub last_delivered_id: String,
    pub entries_read: Option<u64>,
    pub lag: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisXPendingSummary {
    pub count: i64,
    pub min_id: String,
    pub max_id: String,
    pub consumers: Vec<(String, i64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisXPendingEntry {
    pub id: String,
    pub consumer: String,
    pub idle_ms: i64,
    pub delivery_count: i64,
}

/// Unified XPENDING result — summary when no range params, detail list otherwise.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", untagged)]
pub enum RedisXPendingResult {
    Summary(RedisXPendingSummary),
    Entries(Vec<RedisXPendingEntry>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisXClaimEntry {
    pub id: String,
    pub fields: BTreeMap<String, String>,
    pub idle_ms: Option<i64>,
    pub delivery_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisBitmapBit {
    pub offset: u64,
    pub value: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisKeyExtra {
    pub subtype: Option<String>,
    pub stream_info: Option<RedisStreamInfo>,
    pub stream_groups: Option<Vec<RedisStreamGroupInfo>>,
    pub hll_count: Option<u64>,
    pub geo_count: Option<u64>,
    pub bitmap_count: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisStreamView {
    pub entries: Vec<RedisStreamEntry>,
    pub total_len: u64,
    pub start_id: String,
    pub end_id: String,
    pub count: u32,
    pub next_start_id: Option<String>,
    pub stream_info: Option<RedisStreamInfo>,
    pub groups: Vec<RedisStreamGroupInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
pub enum RedisValue {
    String(String),
    Hash(BTreeMap<String, String>),
    List(Vec<String>),
    Set(Vec<String>),
    ZSet(Vec<RedisZSetMember>),
    Stream(Vec<RedisStreamEntry>),
    Json(String),
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisKeyValue {
    pub key: String,
    pub key_type: String,
    pub ttl: i64,
    pub value: RedisValue,
    pub value_total_len: Option<u64>,
    pub value_offset: u64,
    pub is_binary: bool,
    pub extra: Option<RedisKeyExtra>,
    pub object_encoding: Option<String>,
    pub memory_usage: Option<u64>,
    pub object_idletime: Option<i64>,
    pub object_refcount: Option<i64>,
    pub key_exists: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisSetKeyPayload {
    pub key: String,
    pub value: RedisValue,
    pub ttl_seconds: Option<i64>,
    /// SET NX — only set if key does not exist.
    pub set_nx: Option<bool>,
    /// SET XX — only set if key already exists.
    pub set_xx: Option<bool>,
    /// SET PX — expire after this many milliseconds (mutually exclusive with ttl_seconds/EX).
    pub set_px: Option<i64>,
    /// SET KEEPTTL — retain the existing TTL.
    pub set_keepttl: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisMutationResult {
    pub success: bool,
    pub affected: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisListSetItem {
    pub index: usize,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RedisKeyPatchPayload {
    pub key: String,
    pub ttl_seconds: Option<i64>,
    pub hash_set: Option<BTreeMap<String, String>>,
    pub hash_del: Option<Vec<String>>,
    pub set_add: Option<Vec<String>>,
    pub set_rem: Option<Vec<String>>,
    pub zset_add: Option<Vec<RedisZSetMember>>,
    pub zset_rem: Option<Vec<String>>,
    pub list_rpush: Option<Vec<String>>,
    pub list_lpush: Option<Vec<String>>,
    pub list_set: Option<Vec<RedisListSetItem>>,
    pub list_rem: Option<Vec<String>>,
    pub list_lpop: Option<usize>,
    pub list_rpop: Option<usize>,
    pub stream_add: Option<Vec<RedisStreamEntry>>,
    pub stream_del: Option<Vec<String>>,
    pub bitmap_set: Option<Vec<RedisBitmapBit>>,
    pub string_incr_by: Option<String>,
    pub hash_incr_by: Option<BTreeMap<String, String>>,
    pub zset_incr_by: Option<Vec<RedisZSetMember>>,
    pub string_incr_by_int: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisZRangeByScoreResult {
    pub members: Vec<RedisZSetMember>,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisZRangeByLexResult {
    pub members: Vec<String>,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RedisSetOperation {
    Inter,
    Union,
    Diff,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RedisLInsertPosition {
    Before,
    After,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RedisLMoveDirection {
    Left,
    Right,
}

// ── Batch operations types ──────────────────────────────────────────────────

/// A single batch key operation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisBatchKeyOp {
    /// Operation: "del" | "unlink" | "expire" | "persist"
    pub op: String,
    pub key: String,
    /// Only used by "expire" — TTL in seconds.
    pub ttl_seconds: Option<i64>,
}

/// Result of a single batch key operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisBatchKeyOpResult {
    pub key: String,
    pub op: String,
    pub success: bool,
    pub affected: i64,
}

/// Result of a single key in MGET.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisMgetEntry {
    pub key: String,
    pub value: Option<String>,
    pub exists: bool,
}

/// A parsed Redis Cluster node.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisClusterNode {
    pub id: String,
    pub addr: String,
    pub flags: Vec<String>,
    pub master_id: Option<String>,
    pub ping_sent: i64,
    pub pong_recv: i64,
    pub config_epoch: i64,
    pub link_state: String,
    pub slot_range: Option<String>,
}

/// Aggregated cluster information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisClusterInfo {
    pub info: HashMap<String, String>,
    pub nodes: Vec<RedisClusterNode>,
}

fn parse_database(database: Option<&str>) -> Result<i64, String> {
    let Some(raw) = database else {
        return Ok(0);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(0);
    }
    let normalized = trimmed.strip_prefix("db").unwrap_or(trimmed);
    let db = normalized
        .parse::<i64>()
        .map_err(|_| "[VALIDATION_ERROR] Redis database must be a numeric index".to_string())?;
    if !(0..=255).contains(&db) {
        return Err("[VALIDATION_ERROR] Redis database must be between 0 and 255".to_string());
    }
    Ok(db)
}

fn selected_database(form: &ConnectionForm, database: Option<&str>) -> Result<i64, String> {
    match database {
        Some(db) => parse_database(Some(db)),
        None => parse_database(form.database.as_deref()),
    }
}

fn redis_mode(form: &ConnectionForm) -> &str {
    match form.mode.as_deref() {
        Some("standalone") => "standalone",
        Some("cluster") => "cluster",
        Some("sentinel") => "sentinel",
        _ if form
            .host
            .as_deref()
            .map(|host| {
                host.split(',')
                    .filter(|part| !part.trim().is_empty())
                    .count()
                    > 1
            })
            .unwrap_or(false) =>
        {
            "cluster"
        }
        _ => "standalone",
    }
}

fn is_cluster_form(form: &ConnectionForm) -> bool {
    redis_mode(form) == "cluster"
}

fn is_sentinel_form(form: &ConnectionForm) -> bool {
    redis_mode(form) == "sentinel"
}

fn connect_timeout(form: &ConnectionForm) -> Duration {
    Duration::from_millis(
        form.connect_timeout_ms
            .unwrap_or(DEFAULT_CONNECT_TIMEOUT_MS) as u64,
    )
}

fn validate_key(key: &str) -> Result<(), String> {
    if key.trim().is_empty() {
        return Err("[VALIDATION_ERROR] Redis key cannot be empty".to_string());
    }
    Ok(())
}

fn validate_value_for_write(value: &RedisValue) -> Result<(), String> {
    match value {
        RedisValue::Hash(fields) if fields.is_empty() => {
            Err("[VALIDATION_ERROR] Redis hash must contain at least one field".into())
        }
        RedisValue::List(items) if items.is_empty() => {
            Err("[VALIDATION_ERROR] Redis list must contain at least one item".into())
        }
        RedisValue::Set(items) if items.is_empty() => {
            Err("[VALIDATION_ERROR] Redis set must contain at least one member".into())
        }
        RedisValue::ZSet(items) if items.is_empty() => {
            Err("[VALIDATION_ERROR] Redis zset must contain at least one member".into())
        }
        RedisValue::Stream(entries) if entries.is_empty() => {
            Err("[VALIDATION_ERROR] Redis stream must contain at least one entry".into())
        }
        RedisValue::Json(s) => {
            if serde_json::from_str::<serde_json::Value>(s).is_err() {
                return Err("[VALIDATION_ERROR] Invalid JSON".into());
            }
            Ok(())
        }
        RedisValue::None => Err("[VALIDATION_ERROR] Redis value is required".into()),
        _ => Ok(()),
    }
}

fn parse_xrange_value(value: Value) -> Vec<RedisStreamEntry> {
    let arr = match value {
        Value::Array(a) => a,
        _ => return Vec::new(),
    };
    arr.into_iter()
        .filter_map(|entry| {
            let inner = match entry {
                Value::Array(a) if a.len() >= 2 => a,
                _ => return None,
            };
            let id = from_redis_value::<String>(&inner[0]).ok()?;
            let fields_arr = match &inner[1] {
                Value::Array(a) => a,
                _ => return None,
            };
            let mut fields = BTreeMap::new();
            for chunk in fields_arr.chunks_exact(2) {
                let k = from_redis_value::<String>(&chunk[0]).ok()?;
                let v = from_redis_value::<String>(&chunk[1]).ok()?;
                fields.insert(k, v);
            }
            Some(RedisStreamEntry { id, fields })
        })
        .collect()
}

fn parse_stream_info(value: Value) -> Option<RedisStreamInfo> {
    let arr = match value {
        Value::Array(a) => a,
        _ => return None,
    };
    let mut map: HashMap<String, Value> = HashMap::new();
    let mut iter = arr.into_iter();
    while let (Some(k), Some(v)) = (iter.next(), iter.next()) {
        if let Ok(key) = from_redis_value::<String>(&k) {
            map.insert(key.to_lowercase().replace('-', "_"), v);
        }
    }
    let get_u64 = |k: &str| -> u64 {
        map.get(k)
            .and_then(|v| from_redis_value::<u64>(v).ok())
            .unwrap_or(0)
    };
    let get_string = |k: &str| -> String {
        map.get(k)
            .and_then(|v| from_redis_value::<String>(v).ok())
            .unwrap_or_default()
    };
    let parse_entry = |v: &Value| -> Option<RedisStreamEntry> {
        let a = match v {
            Value::Array(a) if a.len() >= 2 => a.clone(),
            _ => return None,
        };
        let id = from_redis_value::<String>(&a[0]).ok()?;
        let fields_arr = match &a[1] {
            Value::Array(a) => a,
            _ => return None,
        };
        let mut fields = BTreeMap::new();
        for chunk in fields_arr.chunks_exact(2) {
            let k = from_redis_value::<String>(&chunk[0]).ok()?;
            let v = from_redis_value::<String>(&chunk[1]).ok()?;
            fields.insert(k, v);
        }
        Some(RedisStreamEntry { id, fields })
    };
    Some(RedisStreamInfo {
        length: get_u64("length"),
        radix_tree_keys: get_u64("radix_tree_keys"),
        radix_tree_nodes: get_u64("radix_tree_nodes"),
        groups: get_u64("groups"),
        last_generated_id: get_string("last_generated_id"),
        first_entry: map.get("first_entry").and_then(parse_entry),
        last_entry: map.get("last_entry").and_then(parse_entry),
    })
}

fn parse_stream_groups(value: Value) -> Vec<RedisStreamGroupInfo> {
    let rows = match value {
        Value::Array(a) => a,
        _ => return Vec::new(),
    };
    rows.into_iter()
        .filter_map(|row| {
            let cols = match row {
                Value::Array(a) => a,
                _ => return None,
            };
            let mut map: HashMap<String, Value> = HashMap::new();
            let mut iter = cols.into_iter();
            while let (Some(k), Some(v)) = (iter.next(), iter.next()) {
                if let Ok(key) = from_redis_value::<String>(&k) {
                    map.insert(key.to_lowercase().replace('-', "_"), v);
                }
            }
            let get_u64 = |k: &str| -> Option<u64> {
                map.get(k).and_then(|v| from_redis_value::<u64>(v).ok())
            };
            let get_string = |k: &str| -> String {
                map.get(k)
                    .and_then(|v| from_redis_value::<String>(v).ok())
                    .unwrap_or_default()
            };
            Some(RedisStreamGroupInfo {
                name: get_string("name"),
                consumers: get_u64("consumers").unwrap_or(0),
                pending: get_u64("pending").unwrap_or(0),
                last_delivered_id: get_string("last_delivered_id"),
                entries_read: get_u64("entries_read"),
                lag: get_u64("lag"),
            })
        })
        .collect()
}

fn build_hll_extra(count: u64) -> RedisKeyExtra {
    RedisKeyExtra {
        subtype: Some("hyperloglog".to_string()),
        stream_info: None,
        stream_groups: None,
        hll_count: Some(count),
        geo_count: None,
        bitmap_count: None,
    }
}

fn build_geo_extra(total: u64) -> RedisKeyExtra {
    RedisKeyExtra {
        subtype: Some("geo".to_string()),
        stream_info: None,
        stream_groups: None,
        hll_count: None,
        geo_count: Some(total),
        bitmap_count: None,
    }
}

fn build_json_module_missing_extra() -> RedisKeyExtra {
    RedisKeyExtra {
        subtype: Some("json-module-missing".to_string()),
        stream_info: None,
        stream_groups: None,
        hll_count: None,
        geo_count: None,
        bitmap_count: None,
    }
}

fn build_stream_extra(
    stream_info: Option<RedisStreamInfo>,
    stream_groups: Vec<RedisStreamGroupInfo>,
) -> RedisKeyExtra {
    RedisKeyExtra {
        subtype: None,
        stream_info,
        stream_groups: Some(stream_groups),
        hll_count: None,
        geo_count: None,
        bitmap_count: None,
    }
}

async fn fetch_stream_view_internal(
    conn: &mut RedisConnection,
    key: &str,
    start_id: &str,
    end_id: &str,
    count: u32,
) -> Result<RedisStreamView, String> {
    let fetch_count = count.saturating_add(1);
    let mut pipe = redis::pipe();
    pipe.cmd("XRANGE")
        .arg(key)
        .arg(start_id)
        .arg(end_id)
        .arg("COUNT")
        .arg(fetch_count)
        .cmd("XINFO")
        .arg("STREAM")
        .arg(key)
        .cmd("XINFO")
        .arg("GROUPS")
        .arg(key);

    let (entries_raw, info_raw, groups_raw): (Value, Value, Value) = conn
        .pipe_query(&mut pipe)
        .await
        .map_err(|e| format!("[REDIS_ERROR] {e}"))?;

    let mut entries = parse_xrange_value(entries_raw);
    let has_more = entries.len() > count as usize;
    if has_more {
        entries.truncate(count as usize);
    }
    let stream_info = parse_stream_info(info_raw);
    let groups = parse_stream_groups(groups_raw);
    let total_len = stream_info.as_ref().map(|info| info.length).unwrap_or(0);
    let next_start_id = if has_more {
        entries.last().map(|entry| format!("({}", entry.id))
    } else {
        None
    };

    Ok(RedisStreamView {
        entries,
        total_len,
        start_id: start_id.to_string(),
        end_id: end_id.to_string(),
        count,
        next_start_id,
        stream_info,
        groups,
    })
}

pub async fn get_stream_range(
    conn: &mut RedisConnection,
    key: String,
    start_id: String,
    count: u32,
) -> Result<Vec<RedisStreamEntry>, String> {
    validate_key(&key)?;
    let count = count.clamp(1, MAX_SCAN_LIMIT);
    let mut cmd = redis::cmd("XRANGE");
    cmd.arg(&key)
        .arg(&start_id)
        .arg("+")
        .arg("COUNT")
        .arg(count);
    let value: Value = conn
        .query(cmd)
        .await
        .map_err(|e| format!("[REDIS_ERROR] {e}"))?;
    Ok(parse_xrange_value(value))
}

pub async fn get_stream_view(
    conn: &mut RedisConnection,
    key: String,
    start_id: String,
    end_id: String,
    count: u32,
) -> Result<RedisStreamView, String> {
    validate_key(&key)?;
    let count = count.clamp(1, MAX_SCAN_LIMIT);
    let normalized_start = if start_id.trim().is_empty() {
        "-".to_string()
    } else {
        start_id.trim().to_string()
    };
    let normalized_end = if end_id.trim().is_empty() {
        "+".to_string()
    } else {
        end_id.trim().to_string()
    };

    fetch_stream_view_internal(conn, &key, &normalized_start, &normalized_end, count).await
}

fn parse_host_port(raw: &str, fallback_port: i64) -> Result<(String, i64), String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("[VALIDATION_ERROR] Redis host is required".to_string());
    }
    if trimmed.starts_with('[') {
        return Ok((trimmed.to_string(), fallback_port));
    }
    let mut parts = trimmed.rsplitn(2, ':');
    let port_part = parts.next().unwrap_or_default();
    let host_part = parts.next();
    if let Some(host) = host_part {
        if !host.is_empty() && port_part.chars().all(|c| c.is_ascii_digit()) {
            let port = port_part
                .parse::<i64>()
                .map_err(|_| "[VALIDATION_ERROR] Redis port is invalid".to_string())?;
            return Ok((host.to_string(), port));
        }
    }
    Ok((trimmed.to_string(), fallback_port))
}

fn build_connection_info_for_host(
    form: &ConnectionForm,
    host: &str,
    db: i64,
) -> Result<ConnectionInfo, String> {
    let (host, port) = parse_host_port(host, form.port.unwrap_or(DEFAULT_REDIS_PORT))?;
    if !(1..=65535).contains(&port) {
        return Err("[VALIDATION_ERROR] Redis port must be between 1 and 65535".to_string());
    }

    let addr = if form.ssl.unwrap_or(false) {
        ConnectionAddr::TcpTls {
            host,
            port: port as u16,
            insecure: false,
            tls_params: None,
        }
    } else {
        ConnectionAddr::Tcp(host, port as u16)
    };

    Ok(ConnectionInfo {
        addr,
        redis: RedisConnectionInfo {
            db,
            username: form
                .username
                .as_deref()
                .filter(|v| !v.is_empty())
                .map(str::to_string),
            password: form
                .password
                .as_deref()
                .filter(|v| !v.is_empty())
                .map(str::to_string),
            protocol: ProtocolVersion::RESP2,
        },
    })
}

fn build_connection_info(form: &ConnectionForm, db: i64) -> Result<ConnectionInfo, String> {
    let host = if let Some(seed_nodes) = form.seed_nodes.as_ref() {
        seed_nodes
            .first()
            .cloned()
            .or_else(|| form.host.clone())
            .ok_or_else(|| "[VALIDATION_ERROR] Redis host is required".to_string())?
    } else {
        form.host
            .clone()
            .ok_or_else(|| "[VALIDATION_ERROR] Redis host is required".to_string())?
    };
    build_connection_info_for_host(form, &host, db)
}

fn build_cluster_nodes(form: &ConnectionForm) -> Result<Vec<ConnectionInfo>, String> {
    let db = selected_database(form, None)?;
    if db != 0 {
        return Err("[VALIDATION_ERROR] Redis Cluster only supports database 0".to_string());
    }
    let nodes: Vec<ConnectionInfo> = form
        .seed_nodes
        .clone()
        .or_else(|| {
            form.host.as_deref().map(|host| {
                host.split(',')
                    .map(str::trim)
                    .filter(|part| !part.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
        })
        .unwrap_or_default()
        .into_iter()
        .map(|part| build_connection_info_for_host(form, &part, 0))
        .collect::<Result<_, _>>()?;
    if nodes.len() < 2 {
        return Err(
            "[VALIDATION_ERROR] Redis Cluster requires at least two seed nodes".to_string(),
        );
    }
    Ok(nodes)
}

fn build_sentinel_node_info(form: &ConnectionForm, db: i64) -> SentinelNodeConnectionInfo {
    let tls_mode = if form.ssl.unwrap_or(false) {
        Some(TlsMode::Secure)
    } else {
        None
    };
    let username = form
        .username
        .as_deref()
        .filter(|v| !v.is_empty())
        .map(str::to_string);
    let password = form
        .password
        .as_deref()
        .filter(|v| !v.is_empty())
        .map(str::to_string);
    SentinelNodeConnectionInfo {
        tls_mode,
        redis_connection_info: Some(RedisConnectionInfo {
            db,
            username,
            password,
            protocol: ProtocolVersion::RESP2,
        }),
    }
}

pub async fn connect(
    form: &ConnectionForm,
    database: Option<&str>,
) -> Result<RedisConnection, String> {
    if is_sentinel_form(form) {
        let sentinel_nodes = form
            .sentinels
            .clone()
            .ok_or("[VALIDATION_ERROR] Sentinel nodes required")?;
        let service_name = form
            .service_name
            .clone()
            .unwrap_or_else(|| "mymaster".to_string());
        let db = selected_database(form, database)?;
        let node_info = build_sentinel_node_info(form, db);

        // Build sentinel node URLs with optional sentinel password
        let sentinel_password = form.sentinel_password.as_deref().filter(|v| !v.is_empty());
        let sentinel_urls: Vec<String> = sentinel_nodes
            .iter()
            .map(|node| {
                if let Some(password) = sentinel_password {
                    format!("redis://:{}@{}", password, node)
                } else {
                    format!("redis://{}", node)
                }
            })
            .collect();

        let mut sentinel = Sentinel::build(sentinel_urls).map_err(|e| conn_failed_error(&e))?;

        let client = sentinel
            .async_master_for(&service_name, Some(&node_info))
            .await
            .map_err(|e| conn_failed_error(&e))?;

        let config = AsyncConnectionConfig::new().set_connection_timeout(connect_timeout(form));
        let conn = client
            .get_multiplexed_async_connection_with_config(&config)
            .await
            .map_err(|e| conn_failed_error(&e))?;
        return Ok(RedisConnection::Standalone(conn));
    }

    if is_cluster_form(form) {
        if let Some(db) = database {
            if parse_database(Some(db))? != 0 {
                return Err("[VALIDATION_ERROR] Redis Cluster only supports database 0".to_string());
            }
        }
        let nodes = build_cluster_nodes(form)?;
        let client = ClusterClient::builder(nodes)
            .connection_timeout(connect_timeout(form))
            .build()
            .map_err(|e| conn_failed_error(&e))?;
        let conn = client
            .get_async_connection()
            .await
            .map_err(|e| conn_failed_error(&e))?;
        return Ok(RedisConnection::Cluster(Arc::new(TokioMutex::new(conn))));
    }

    let db = selected_database(form, database)?;
    let info = build_connection_info(form, db)?;
    let client = redis::Client::open(info).map_err(|e| conn_failed_error(&e))?;
    let config = AsyncConnectionConfig::new().set_connection_timeout(connect_timeout(form));
    let conn = client
        .get_multiplexed_async_connection_with_config(&config)
        .await
        .map_err(|e| conn_failed_error(&e))?;
    Ok(RedisConnection::Standalone(conn))
}

async fn query_on<T: FromRedisValue, C: ConnectionLike + Send + Sync>(
    conn: &mut C,
    cmd: Cmd,
) -> Result<T, String> {
    cmd.query_async::<T>(conn)
        .await
        .map_err(|e| format!("[REDIS_ERROR] {e}"))
}

pub async fn ping(conn: &mut RedisConnection) -> Result<(), String> {
    conn.query::<String>(redis::cmd("PING"))
        .await
        .map(|_| ())
        .map_err(|e| conn_failed_error(&e))
}

pub fn list_databases(
    form: &ConnectionForm,
    db_count: i64,
) -> Result<Vec<RedisDatabaseInfo>, String> {
    if is_cluster_form(form) {
        build_cluster_nodes(form)?;
        return Ok(vec![RedisDatabaseInfo {
            index: 0,
            name: "db0".to_string(),
            selected: true,
            key_count: None,
        }]);
    }

    // Sentinel resolves to a master node, so database selection works like standalone.
    let selected = selected_database(form, None)?;
    let db_count = db_count.clamp(1, 256);
    Ok((0..db_count)
        .map(|index| RedisDatabaseInfo {
            index,
            name: format!("db{index}"),
            selected: index == selected,
            key_count: None,
        })
        .collect())
}

pub async fn server_info(conn: &mut RedisConnection) -> Result<RedisServerInfo, String> {
    let info_str: String = conn
        .query(redis::cmd("INFO"))
        .await
        .map_err(|e| format!("[REDIS_ERROR] {e}"))?;

    let mut sections: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut current_section = String::new();

    for line in info_str.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            if let Some(name) = line.strip_prefix("# ") {
                current_section = name.to_string();
            }
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            sections
                .entry(current_section.clone())
                .or_default()
                .insert(key.to_string(), value.to_string());
        }
    }

    let dbsize: u64 = conn
        .query(redis::cmd("DBSIZE"))
        .await
        .map_err(|e| format!("[REDIS_ERROR] {e}"))?;

    Ok(RedisServerInfo { sections, dbsize })
}

pub async fn server_config(conn: &mut RedisConnection) -> Result<HashMap<String, String>, String> {
    let mut cmd = redis::cmd("CONFIG");
    cmd.arg("GET").arg("*");
    let values: Vec<String> = conn
        .query(cmd)
        .await
        .map_err(|e| format!("[REDIS_ERROR] {e}"))?;

    let mut config = HashMap::new();
    let mut iter = values.into_iter();
    while let Some(key) = iter.next() {
        if let Some(value) = iter.next() {
            config.insert(key, value);
        }
    }
    Ok(config)
}

pub async fn slowlog_get(
    conn: &mut RedisConnection,
    count: i64,
) -> Result<Vec<RedisSlowlogEntry>, String> {
    let mut cmd = redis::cmd("SLOWLOG");
    cmd.arg("GET").arg(count.max(1));
    let raw: Vec<Vec<Value>> = conn
        .query(cmd)
        .await
        .map_err(|e| format!("[REDIS_ERROR] {e}"))?;

    let mut entries = Vec::new();
    for item in raw {
        if item.len() < 4 {
            continue;
        }
        let id = match &item[0] {
            Value::Int(v) => *v as u64,
            _ => continue,
        };
        let timestamp = match &item[1] {
            Value::Int(v) => *v,
            _ => continue,
        };
        let duration_ms = match &item[2] {
            Value::Int(v) => *v as u64,
            _ => continue,
        };
        let command = match &item[3] {
            Value::Array(parts) => parts
                .iter()
                .filter_map(|p| match p {
                    Value::BulkString(b) => String::from_utf8(b.clone()).ok(),
                    Value::SimpleString(s) => Some(s.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(" "),
            _ => format!("{:?}", item[3]),
        };
        entries.push(RedisSlowlogEntry {
            id,
            timestamp,
            duration_ms,
            command,
        });
    }
    Ok(entries)
}

fn encode_cluster_scan_state(cursors: &HashMap<String, u64>) -> Result<String, String> {
    let json = serde_json::to_string(cursors).map_err(|e| format!("[REDIS_SCAN_ERROR] {e}"))?;
    Ok(base64::prelude::BASE64_STANDARD.encode(json.as_bytes()))
}

fn decode_cluster_scan_state(s: &str) -> Result<HashMap<String, u64>, String> {
    let bytes = base64::prelude::BASE64_STANDARD
        .decode(s)
        .map_err(|e| format!("[REDIS_SCAN_ERROR] Invalid cursor: {e}"))?;
    let json =
        String::from_utf8(bytes).map_err(|e| format!("[REDIS_SCAN_ERROR] Invalid cursor: {e}"))?;
    serde_json::from_str(&json).map_err(|e| format!("[REDIS_SCAN_ERROR] Invalid cursor: {e}"))
}

async fn get_cluster_master_nodes(
    conn: &mut RedisConnection,
) -> Result<Vec<(String, u16)>, String> {
    let mut cmd = redis::cmd("CLUSTER");
    cmd.arg("SLOTS");
    let value: Value = conn.query(cmd).await?;

    let slots = match value {
        Value::Array(arr) => arr,
        _ => return Err("[REDIS_SCAN_ERROR] Unexpected CLUSTER SLOTS response".to_string()),
    };

    let mut masters = Vec::new();
    for slot in slots {
        let slot_arr = match slot {
            Value::Array(arr) => arr,
            _ => continue,
        };
        if slot_arr.len() < 3 {
            continue;
        }
        let master_info = match &slot_arr[2] {
            Value::Array(info) => info,
            _ => continue,
        };
        if master_info.len() < 2 {
            continue;
        }
        let host = from_redis_value::<String>(&master_info[0])
            .map_err(|e| format!("[REDIS_SCAN_ERROR] {e}"))?;
        let port = from_redis_value::<u16>(&master_info[1])
            .map_err(|e| format!("[REDIS_SCAN_ERROR] {e}"))?;
        masters.push((host, port));
    }

    masters.sort();
    masters.dedup();
    Ok(masters)
}

fn parse_node_addr(addr: &str) -> Result<(&str, u16), String> {
    let mut parts = addr.rsplitn(2, ':');
    let port_part = parts
        .next()
        .ok_or_else(|| "[REDIS_SCAN_ERROR] Invalid node addr".to_string())?;
    let host_part = parts
        .next()
        .ok_or_else(|| "[REDIS_SCAN_ERROR] Invalid node addr".to_string())?;
    let port = port_part
        .parse::<u16>()
        .map_err(|_| "[REDIS_SCAN_ERROR] Invalid node port".to_string())?;
    Ok((host_part, port))
}

fn is_dangerous_wildcard(pattern: &str) -> bool {
    let trimmed = pattern.trim();
    if trimmed.is_empty() {
        return true;
    }
    !trimmed.chars().any(|c| c.is_alphanumeric())
}

/// Run SCAN on a single cluster node and return the next cursor + keys.
/// `cursor` is the raw value from the cursors map: 0 means "finished",
/// u64::MAX means "never scanned yet" (translated to 0 for the actual SCAN call).
async fn scan_one_cluster_node(
    conn: &mut RedisConnection,
    addr: &str,
    cursor: u64,
    pattern: &str,
    count: u32,
) -> Result<(u64, Vec<String>), String> {
    if cursor == 0 {
        return Ok((0, Vec::new()));
    }
    let effective_cursor = if cursor == u64::MAX { 0 } else { cursor };
    let (host, port) = parse_node_addr(addr)?;
    let mut cmd = redis::cmd("SCAN");
    cmd.arg(effective_cursor)
        .arg("MATCH")
        .arg(pattern)
        .arg("COUNT")
        .arg(count);
    conn.query_on_node(host, port, cmd)
        .await
        .map_err(|e| format!("[REDIS_SCAN_ERROR] {e}"))
}

async fn scan_cluster_keys(
    conn: &mut RedisConnection,
    state: Option<&str>,
    pattern: &str,
    count: u32,
) -> Result<(Vec<String>, String, bool), String> {
    if is_dangerous_wildcard(pattern) {
        return Err("[VALIDATION_ERROR] Cluster scan requires a non-wildcard pattern".to_string());
    }
    let masters = get_cluster_master_nodes(conn).await?;

    let mut cursors: HashMap<String, u64> = match state {
        Some(s) => decode_cluster_scan_state(s)?,
        None => HashMap::new(),
    };

    // Seed any newly-discovered masters.  u64::MAX means "never scanned yet";
    // it is translated to 0 when passed to SCAN so the first call starts
    // every master from the beginning.
    for (host, port) in &masters {
        cursors.entry(format!("{host}:{port}")).or_insert(u64::MAX);
    }

    let mut keys: Vec<String> = Vec::new();
    let mut addresses: Vec<String> = cursors.keys().cloned().collect();
    addresses.sort();

    // Scan every master that still has work to do once per call.
    for addr in &addresses {
        let cursor = cursors.get(addr).copied().unwrap_or(u64::MAX);
        let (next_cursor, node_keys) =
            scan_one_cluster_node(conn, addr, cursor, pattern, count).await?;
        keys.extend(node_keys);
        cursors.insert(addr.clone(), next_cursor);
    }

    // If we still haven't reached the limit and some nodes remain unfinished,
    // perform one more round to be more eager.
    if keys.len() < count as usize {
        for addr in &addresses {
            let cursor = cursors.get(addr).copied().unwrap_or(u64::MAX);
            let (next_cursor, node_keys) =
                scan_one_cluster_node(conn, addr, cursor, pattern, count).await?;
            keys.extend(node_keys);
            cursors.insert(addr.clone(), next_cursor);
            if keys.len() >= count as usize {
                break;
            }
        }
    }

    keys.sort();
    keys.truncate(count as usize);

    let is_partial = cursors.values().any(|c| *c != 0);
    let next_state = encode_cluster_scan_state(&cursors)?;
    Ok((keys, next_state, is_partial))
}

/// Query TYPE and TTL for a list of keys.
/// In cluster mode keys may span different slots, so pipeline is not allowed.
/// In standalone mode we use a pipeline for efficiency.
async fn query_key_metas(
    conn: &mut RedisConnection,
    keys: Vec<String>,
) -> Result<Vec<RedisKeyInfo>, String> {
    if keys.is_empty() {
        return Ok(Vec::new());
    }

    if conn.is_cluster() {
        let mut out = Vec::with_capacity(keys.len());
        for key in keys {
            let mut type_cmd = redis::cmd("TYPE");
            type_cmd.arg(&key);
            let key_type = conn
                .query(type_cmd)
                .await
                .unwrap_or_else(|_| "unknown".to_string());
            let mut ttl_cmd = redis::cmd("TTL");
            ttl_cmd.arg(&key);
            let ttl = conn.query(ttl_cmd).await.unwrap_or(-2);
            out.push(RedisKeyInfo { key, key_type, ttl });
        }
        Ok(out)
    } else {
        let mut pipe = redis::pipe();
        for key in &keys {
            pipe.cmd("TYPE").arg(key);
            pipe.cmd("TTL").arg(key);
        }
        let results: Vec<Value> = conn
            .pipe_query(&mut pipe)
            .await
            .map_err(|e| format!("[REDIS_SCAN_ERROR] {e}"))?;
        Ok(keys
            .into_iter()
            .enumerate()
            .map(|(i, key)| {
                let key_type = from_redis_value(results.get(i * 2).unwrap_or(&Value::Nil))
                    .unwrap_or_else(|_| "unknown".to_string());
                let ttl =
                    from_redis_value(results.get(i * 2 + 1).unwrap_or(&Value::Nil)).unwrap_or(-2);
                RedisKeyInfo { key, key_type, ttl }
            })
            .collect())
    }
}

pub async fn scan_keys(
    conn: &mut RedisConnection,
    cursor: Option<String>,
    pattern: Option<String>,
    limit: Option<u32>,
) -> Result<RedisScanResponse, String> {
    let count = limit.unwrap_or(DEFAULT_SCAN_LIMIT).clamp(1, MAX_SCAN_LIMIT);
    let match_pattern = pattern
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .unwrap_or("*");

    let (next_cursor, is_partial, keys): (String, bool, Vec<String>) = if conn.is_cluster() {
        let (keys, next_state, partial) =
            scan_cluster_keys(conn, cursor.as_deref(), match_pattern, count).await?;
        (next_state, partial, keys)
    } else {
        let scan_cursor: u64 = cursor
            .as_deref()
            .unwrap_or("0")
            .parse()
            .map_err(|_| "[VALIDATION_ERROR] Invalid cursor".to_string())?;
        let mut cmd = redis::cmd("SCAN");
        cmd.arg(scan_cursor)
            .arg("MATCH")
            .arg(match_pattern)
            .arg("COUNT")
            .arg(count);
        let (next_cursor, keys): (u64, Vec<String>) = conn
            .query(cmd)
            .await
            .map_err(|e| format!("[REDIS_SCAN_ERROR] {e}"))?;
        let partial = next_cursor != 0;
        (next_cursor.to_string(), partial, keys)
    };

    let out = if keys.is_empty() {
        Vec::new()
    } else {
        query_key_metas(conn, keys).await?
    };

    Ok(RedisScanResponse {
        cursor: next_cursor,
        keys: out,
        is_partial,
    })
}

pub async fn get_key(conn: &mut RedisConnection, key: String) -> Result<RedisKeyValue, String> {
    validate_key(&key)?;

    let mut pipe1 = redis::pipe();
    pipe1.cmd("TYPE").arg(&key).cmd("TTL").arg(&key);
    let (key_type, ttl): (String, i64) = conn
        .pipe_query(&mut pipe1)
        .await
        .map_err(|e| format!("[REDIS_ERROR] {e}"))?;

    let page = PAGE_SIZE - 1;
    let (value, value_total_len, value_offset, is_binary, extra): (
        RedisValue,
        Option<u64>,
        u64,
        bool,
        Option<RedisKeyExtra>,
    ) = match key_type.as_str() {
        "none" => (RedisValue::None, None, 0, false, None),
        "string" => {
            // HyperLogLog detection must happen before GET, because HLL
            // internal encoding is binary and would set is_binary=true.
            let mut extra = None;
            let mut hll_cmd = redis::cmd("PFCOUNT");
            hll_cmd.arg(&key);
            match conn.query::<i64>(hll_cmd).await {
                Ok(count) if count >= 0 => {
                    extra = Some(build_hll_extra(count as u64));
                }
                _ => {}
            }

            let mut cmd = redis::cmd("GET");
            cmd.arg(&key);
            let bytes: Vec<u8> = conn.query(cmd).await.unwrap_or_default();
            let (text, is_binary) = match String::from_utf8(bytes) {
                Ok(s) => (s, false),
                Err(e) => {
                    let encoded = base64::prelude::BASE64_STANDARD.encode(e.into_bytes());
                    (encoded, true)
                }
            };

            (RedisValue::String(text), None, 0, is_binary, extra)
        }
        "hash" => {
            let mut pipe = redis::pipe();
            pipe.cmd("HLEN")
                .arg(&key)
                .cmd("HSCAN")
                .arg(&key)
                .arg(0u64)
                .arg("COUNT")
                .arg(PAGE_SIZE);
            let (total, (next_cursor, fields)): (u64, (u64, BTreeMap<String, String>)) =
                conn.pipe_query(&mut pipe).await.unwrap_or_default();
            (
                RedisValue::Hash(fields),
                Some(total),
                next_cursor,
                false,
                None,
            )
        }
        "list" => {
            let mut pipe = redis::pipe();
            pipe.cmd("LLEN")
                .arg(&key)
                .cmd("LRANGE")
                .arg(&key)
                .arg(0)
                .arg(page);
            let (total, items): (u64, Vec<String>) =
                conn.pipe_query(&mut pipe).await.unwrap_or_default();
            let next_offset = (items.len() as u64).min(total);
            (
                RedisValue::List(items),
                Some(total),
                next_offset,
                false,
                None,
            )
        }
        "set" => {
            let mut pipe = redis::pipe();
            pipe.cmd("SCARD")
                .arg(&key)
                .cmd("SSCAN")
                .arg(&key)
                .arg(0u64)
                .arg("COUNT")
                .arg(PAGE_SIZE);
            let (total, (next_cursor, members)): (u64, (u64, Vec<String>)) =
                conn.pipe_query(&mut pipe).await.unwrap_or_default();
            (
                RedisValue::Set(members),
                Some(total),
                next_cursor,
                false,
                None,
            )
        }
        "zset" => {
            let mut pipe = redis::pipe();
            pipe.cmd("ZCARD")
                .arg(&key)
                .cmd("ZRANGE")
                .arg(&key)
                .arg(0)
                .arg(page)
                .arg("WITHSCORES");
            let (total, members): (u64, Vec<(String, f64)>) =
                conn.pipe_query(&mut pipe).await.unwrap_or_default();
            let next_offset = (members.len() as u64).min(total);
            let mut extra = None;
            if let Some(first) = members.first() {
                let mut geo_cmd = redis::cmd("GEOPOS");
                geo_cmd.arg(&key).arg(&first.0);
                if let Ok(positions) = conn.query::<Vec<Option<(f64, f64)>>>(geo_cmd).await {
                    if positions.iter().any(|p| p.is_some()) {
                        extra = Some(build_geo_extra(total));
                    }
                }
            }
            (
                RedisValue::ZSet(
                    members
                        .into_iter()
                        .map(|(member, score)| RedisZSetMember { member, score })
                        .collect(),
                ),
                Some(total),
                next_offset,
                false,
                extra,
            )
        }
        "stream" => {
            let view = fetch_stream_view_internal(conn, &key, "-", "+", PAGE_SIZE as u32).await?;
            let extra = Some(build_stream_extra(
                view.stream_info.clone(),
                view.groups.clone(),
            ));
            (
                RedisValue::Stream(view.entries),
                Some(view.total_len),
                view.total_len.min(PAGE_SIZE as u64),
                false,
                extra,
            )
        }
        "ReJSON-RL" | "json" | "JSON" => {
            let mut cmd = redis::cmd("JSON.GET");
            cmd.arg(&key).arg(".");
            match conn.query::<String>(cmd).await {
                Ok(json_str) => (RedisValue::Json(json_str), None, 0, false, None),
                Err(e) if e.to_string().to_lowercase().contains("unknown command") => {
                    let mut cmd = redis::cmd("GET");
                    cmd.arg(&key);
                    let bytes: Vec<u8> = conn.query(cmd).await.unwrap_or_default();
                    let (text, is_binary) = match String::from_utf8(bytes) {
                        Ok(s) => (s, false),
                        Err(e) => {
                            let encoded = base64::prelude::BASE64_STANDARD.encode(e.into_bytes());
                            (encoded, true)
                        }
                    };
                    let extra = Some(build_json_module_missing_extra());
                    (RedisValue::Json(text), None, 0, is_binary, extra)
                }
                Err(e) => return Err(format!("[REDIS_ERROR] {e}")),
            }
        }
        other => {
            return Err(format!(
                "[UNSUPPORTED] Redis type '{other}' is not supported"
            ))
        }
    };

    let object_encoding = {
        let mut cmd = redis::cmd("OBJECT");
        cmd.arg("ENCODING").arg(&key);
        conn.query::<String>(cmd).await.ok()
    };

    let memory_usage = {
        let mut cmd = redis::cmd("MEMORY");
        cmd.arg("USAGE").arg(&key);
        conn.query::<i64>(cmd).await.ok().map(|v| v.max(0) as u64)
    };

    let object_idletime = {
        let mut cmd = redis::cmd("OBJECT");
        cmd.arg("IDLETIME").arg(&key);
        conn.query::<i64>(cmd).await.ok()
    };

    let object_refcount = {
        let mut cmd = redis::cmd("OBJECT");
        cmd.arg("REFCOUNT").arg(&key);
        conn.query::<i64>(cmd).await.ok()
    };

    let key_exists = {
        let mut cmd = redis::cmd("EXISTS");
        cmd.arg(&key);
        conn.query::<i64>(cmd).await.ok().map(|v| v > 0)
    };

    Ok(RedisKeyValue {
        key,
        key_type,
        ttl,
        value,
        value_total_len,
        value_offset,
        is_binary,
        extra,
        object_encoding,
        memory_usage,
        object_idletime,
        object_refcount,
        key_exists,
    })
}

pub async fn get_key_page(
    conn: &mut RedisConnection,
    key: String,
    offset: u64,
    limit: u32,
) -> Result<RedisKeyValue, String> {
    validate_key(&key)?;
    let limit = limit.clamp(1, MAX_SCAN_LIMIT);

    let mut pipe1 = redis::pipe();
    pipe1.cmd("TYPE").arg(&key).cmd("TTL").arg(&key);
    let (key_type, ttl): (String, i64) = conn
        .pipe_query(&mut pipe1)
        .await
        .map_err(|e| format!("[REDIS_ERROR] {e}"))?;

    let end = offset.saturating_add(limit as u64).saturating_sub(1);

    let (value, value_total_len, value_offset, extra): (
        RedisValue,
        Option<u64>,
        u64,
        Option<RedisKeyExtra>,
    ) = match key_type.as_str() {
        "list" => {
            let mut pipe = redis::pipe();
            pipe.cmd("LLEN")
                .arg(&key)
                .cmd("LRANGE")
                .arg(&key)
                .arg(offset)
                .arg(end);
            let (total, items): (u64, Vec<String>) =
                conn.pipe_query(&mut pipe).await.unwrap_or_default();
            let next_offset = offset.saturating_add(items.len() as u64).min(total);
            (RedisValue::List(items), Some(total), next_offset, None)
        }
        "zset" => {
            let mut pipe = redis::pipe();
            pipe.cmd("ZCARD")
                .arg(&key)
                .cmd("ZRANGE")
                .arg(&key)
                .arg(offset)
                .arg(end)
                .arg("WITHSCORES");
            let (total, members): (u64, Vec<(String, f64)>) =
                conn.pipe_query(&mut pipe).await.unwrap_or_default();
            let next_offset = offset.saturating_add(members.len() as u64).min(total);
            let mut extra = None;
            if let Some(first) = members.first() {
                let mut geo_cmd = redis::cmd("GEOPOS");
                geo_cmd.arg(&key).arg(&first.0);
                if let Ok(positions) = conn.query::<Vec<Option<(f64, f64)>>>(geo_cmd).await {
                    if positions.iter().any(|p| p.is_some()) {
                        extra = Some(build_geo_extra(total));
                    }
                }
            }
            (
                RedisValue::ZSet(
                    members
                        .into_iter()
                        .map(|(member, score)| RedisZSetMember { member, score })
                        .collect(),
                ),
                Some(total),
                next_offset,
                extra,
            )
        }
        "hash" => {
            let mut pipe = redis::pipe();
            pipe.cmd("HLEN")
                .arg(&key)
                .cmd("HSCAN")
                .arg(&key)
                .arg(offset)
                .arg("COUNT")
                .arg(limit);
            let (total, (next_cursor, fields)): (u64, (u64, BTreeMap<String, String>)) =
                conn.pipe_query(&mut pipe).await.unwrap_or_default();
            (RedisValue::Hash(fields), Some(total), next_cursor, None)
        }
        "set" => {
            let mut pipe = redis::pipe();
            pipe.cmd("SCARD")
                .arg(&key)
                .cmd("SSCAN")
                .arg(&key)
                .arg(offset)
                .arg("COUNT")
                .arg(limit);
            let (total, (next_cursor, members)): (u64, (u64, Vec<String>)) =
                conn.pipe_query(&mut pipe).await.unwrap_or_default();
            (RedisValue::Set(members), Some(total), next_cursor, None)
        }
        "string" | "none" | "stream" | "ReJSON-RL" | "json" | "JSON" => {
            return get_key(conn, key).await;
        }
        other => {
            return Err(format!(
                "[UNSUPPORTED] Redis type '{other}' is not supported"
            ))
        }
    };

    let object_encoding = {
        let mut cmd = redis::cmd("OBJECT");
        cmd.arg("ENCODING").arg(&key);
        conn.query::<String>(cmd).await.ok()
    };

    let memory_usage = {
        let mut cmd = redis::cmd("MEMORY");
        cmd.arg("USAGE").arg(&key);
        conn.query::<i64>(cmd).await.ok().map(|v| v.max(0) as u64)
    };

    let object_idletime = {
        let mut cmd = redis::cmd("OBJECT");
        cmd.arg("IDLETIME").arg(&key);
        conn.query::<i64>(cmd).await.ok()
    };

    let object_refcount = {
        let mut cmd = redis::cmd("OBJECT");
        cmd.arg("REFCOUNT").arg(&key);
        conn.query::<i64>(cmd).await.ok()
    };

    let key_exists = {
        let mut cmd = redis::cmd("EXISTS");
        cmd.arg(&key);
        conn.query::<i64>(cmd).await.ok().map(|v| v > 0)
    };

    Ok(RedisKeyValue {
        key,
        key_type,
        ttl,
        value,
        value_total_len,
        value_offset,
        is_binary: false,
        extra,
        object_encoding,
        memory_usage,
        object_idletime,
        object_refcount,
        key_exists,
    })
}

pub async fn set_key(
    conn: &mut RedisConnection,
    payload: RedisSetKeyPayload,
) -> Result<RedisMutationResult, String> {
    validate_key(&payload.key)?;
    validate_value_for_write(&payload.value)?;
    let mut del_cmd = redis::cmd("DEL");
    del_cmd.arg(&payload.key);
    let _: i64 = conn.query(del_cmd).await.unwrap_or(0);

    let ttl_handled_atomically = matches!(payload.value, RedisValue::String(_));
    match payload.value {
        RedisValue::String(value) => {
            let mut cmd = redis::cmd("SET");
            cmd.arg(&payload.key).arg(value);
            // Atomic SET options: PX (ms) takes precedence over EX (s).
            if let Some(px) = payload.set_px {
                if px > 0 {
                    cmd.arg("PX").arg(px);
                }
            } else if let Some(ttl) = payload.ttl_seconds {
                if ttl > 0 {
                    cmd.arg("EX").arg(ttl);
                }
            }
            if payload.set_keepttl.unwrap_or(false) {
                cmd.arg("KEEPTTL");
            }
            if payload.set_nx.unwrap_or(false) {
                cmd.arg("NX");
            } else if payload.set_xx.unwrap_or(false) {
                cmd.arg("XX");
            }
            conn.query::<()>(cmd).await?;
        }
        RedisValue::Hash(fields) => {
            let mut cmd = redis::cmd("HSET");
            cmd.arg(&payload.key);
            for (field, value) in fields {
                cmd.arg(field).arg(value);
            }
            conn.query::<i64>(cmd).await?;
        }
        RedisValue::List(items) => {
            let mut cmd = redis::cmd("RPUSH");
            cmd.arg(&payload.key).arg(items);
            conn.query::<i64>(cmd).await?;
        }
        RedisValue::Set(items) => {
            let mut cmd = redis::cmd("SADD");
            cmd.arg(&payload.key).arg(items);
            conn.query::<i64>(cmd).await?;
        }
        RedisValue::ZSet(items) => {
            for item in items {
                let mut cmd = redis::cmd("ZADD");
                cmd.arg(&payload.key).arg(item.score).arg(item.member);
                conn.query::<i64>(cmd).await?;
            }
        }
        RedisValue::Stream(entries) => {
            for entry in entries {
                let mut cmd = redis::cmd("XADD");
                cmd.arg(&payload.key).arg(&entry.id);
                for (field, value) in entry.fields {
                    cmd.arg(field).arg(value);
                }
                conn.query::<String>(cmd).await?;
            }
        }
        RedisValue::Json(json_str) => {
            let mut cmd = redis::cmd("JSON.SET");
            cmd.arg(&payload.key).arg(".").arg(json_str);
            conn.query::<()>(cmd).await?;
        }
        RedisValue::None => unreachable!("validated above"),
    }

    if !ttl_handled_atomically {
        if let Some(ttl) = payload.ttl_seconds {
            if ttl > 0 {
                let mut cmd = redis::cmd("EXPIRE");
                cmd.arg(&payload.key).arg(ttl);
                conn.query::<bool>(cmd).await?;
            }
        }
    }

    Ok(RedisMutationResult {
        success: true,
        affected: 1,
    })
}

pub async fn delete_key(
    conn: &mut RedisConnection,
    key: String,
) -> Result<RedisMutationResult, String> {
    validate_key(&key)?;
    let mut cmd = redis::cmd("DEL");
    cmd.arg(key);
    let affected: i64 = conn.query(cmd).await?;
    Ok(RedisMutationResult {
        success: true,
        affected,
    })
}

pub async fn patch_key(
    conn: &mut RedisConnection,
    payload: RedisKeyPatchPayload,
) -> Result<RedisMutationResult, String> {
    validate_key(&payload.key)?;
    let key = &payload.key;

    if let Some(fields) = payload.hash_set {
        if !fields.is_empty() {
            let mut cmd = redis::cmd("HSET");
            cmd.arg(key);
            for (f, v) in fields {
                cmd.arg(f).arg(v);
            }
            conn.query::<i64>(cmd).await?;
        }
    }
    if let Some(fields) = payload.hash_del {
        if !fields.is_empty() {
            let mut cmd = redis::cmd("HDEL");
            cmd.arg(key);
            for f in fields {
                cmd.arg(f);
            }
            conn.query::<i64>(cmd).await?;
        }
    }
    if let Some(members) = payload.set_add {
        if !members.is_empty() {
            let mut cmd = redis::cmd("SADD");
            cmd.arg(key).arg(members);
            conn.query::<i64>(cmd).await?;
        }
    }
    if let Some(members) = payload.set_rem {
        if !members.is_empty() {
            let mut cmd = redis::cmd("SREM");
            cmd.arg(key).arg(members);
            conn.query::<i64>(cmd).await?;
        }
    }
    if let Some(members) = payload.zset_add {
        if !members.is_empty() {
            let mut cmd = redis::cmd("ZADD");
            cmd.arg(key);
            for m in members {
                cmd.arg(m.score).arg(m.member);
            }
            conn.query::<i64>(cmd).await?;
        }
    }
    if let Some(members) = payload.zset_rem {
        if !members.is_empty() {
            let mut cmd = redis::cmd("ZREM");
            cmd.arg(key).arg(members);
            conn.query::<i64>(cmd).await?;
        }
    }
    if let Some(items) = payload.list_rpush {
        if !items.is_empty() {
            let mut cmd = redis::cmd("RPUSH");
            cmd.arg(key).arg(items);
            conn.query::<i64>(cmd).await?;
        }
    }
    if let Some(items) = payload.list_lpush {
        if !items.is_empty() {
            let mut cmd = redis::cmd("LPUSH");
            cmd.arg(key).arg(items);
            conn.query::<i64>(cmd).await?;
        }
    }
    if let Some(items) = payload.list_set {
        for item in items {
            let mut cmd = redis::cmd("LSET");
            cmd.arg(key).arg(item.index).arg(item.value);
            conn.query::<()>(cmd).await?;
        }
    }
    if let Some(values) = payload.list_rem {
        for value in values {
            let mut cmd = redis::cmd("LREM");
            cmd.arg(key).arg(0).arg(value);
            conn.query::<i64>(cmd).await?;
        }
    }
    if let Some(count) = payload.list_lpop {
        if count > 0 {
            let mut cmd = redis::cmd("LPOP");
            cmd.arg(key).arg(count);
            conn.query::<()>(cmd).await?;
        }
    }
    if let Some(count) = payload.list_rpop {
        if count > 0 {
            let mut cmd = redis::cmd("RPOP");
            cmd.arg(key).arg(count);
            conn.query::<()>(cmd).await?;
        }
    }
    if let Some(entries) = payload.stream_add {
        if !entries.is_empty() {
            for entry in entries {
                let mut cmd = redis::cmd("XADD");
                cmd.arg(key).arg(&entry.id);
                for (field, value) in entry.fields {
                    cmd.arg(field).arg(value);
                }
                conn.query::<String>(cmd).await?;
            }
        }
    }
    if let Some(ids) = payload.stream_del {
        if !ids.is_empty() {
            let mut cmd = redis::cmd("XDEL");
            cmd.arg(key).arg(ids);
            conn.query::<i64>(cmd).await?;
        }
    }
    if let Some(bits) = payload.bitmap_set {
        for bit in bits {
            let mut cmd = redis::cmd("SETBIT");
            cmd.arg(key)
                .arg(bit.offset)
                .arg(if bit.value { 1 } else { 0 });
            conn.query::<i64>(cmd).await?;
        }
    }
    if let Some(ref amount) = payload.string_incr_by {
        let mut cmd = redis::cmd("INCRBYFLOAT");
        cmd.arg(key).arg(amount);
        conn.query::<String>(cmd).await?;
    }
    if let Some(amount) = payload.string_incr_by_int {
        let mut cmd = redis::cmd("INCRBY");
        cmd.arg(key).arg(amount);
        conn.query::<i64>(cmd).await?;
    }
    if let Some(ref fields) = payload.hash_incr_by {
        for (field, amount) in fields {
            let mut cmd = redis::cmd("HINCRBYFLOAT");
            cmd.arg(key).arg(field).arg(amount);
            conn.query::<String>(cmd).await?;
        }
    }
    if let Some(ref members) = payload.zset_incr_by {
        for m in members {
            let mut cmd = redis::cmd("ZINCRBY");
            cmd.arg(key).arg(m.score).arg(&m.member);
            conn.query::<String>(cmd).await?;
        }
    }

    match payload.ttl_seconds {
        Some(ttl) if ttl > 0 => {
            let mut cmd = redis::cmd("EXPIRE");
            cmd.arg(key).arg(ttl);
            conn.query::<bool>(cmd).await?;
        }
        Some(_) => {
            // Caller sends 0 or negative to explicitly remove TTL.
            let mut cmd = redis::cmd("PERSIST");
            cmd.arg(key);
            conn.query::<bool>(cmd).await?;
        }
        None => {
            // None means "leave TTL unchanged" — no action.
        }
    }

    Ok(RedisMutationResult {
        success: true,
        affected: 1,
    })
}

pub async fn rename_key(
    conn: &mut RedisConnection,
    old_key: String,
    new_key: String,
    force: bool,
) -> Result<RedisMutationResult, String> {
    validate_key(&old_key)?;
    validate_key(&new_key)?;
    let cmd_name = if force { "RENAME" } else { "RENAMENX" };
    let mut cmd = redis::cmd(cmd_name);
    cmd.arg(&old_key).arg(&new_key);
    let renamed: i64 = conn.query(cmd).await?;
    if renamed == 0 && !force {
        return Err(format!(
            "[REDIS_ERROR] Key '{}' already exists. RENAMENX refused to overwrite.",
            new_key
        ));
    }
    Ok(RedisMutationResult {
        success: true,
        affected: 1,
    })
}

pub async fn set_ttl(
    conn: &mut RedisConnection,
    key: String,
    ttl_seconds: Option<i64>,
) -> Result<RedisMutationResult, String> {
    validate_key(&key)?;
    let changed: bool = match ttl_seconds {
        Some(ttl) if ttl > 0 => {
            let mut cmd = redis::cmd("EXPIRE");
            cmd.arg(key).arg(ttl);
            conn.query(cmd).await?
        }
        _ => {
            let mut cmd = redis::cmd("PERSIST");
            cmd.arg(key);
            conn.query(cmd).await?
        }
    };
    Ok(RedisMutationResult {
        success: true,
        affected: if changed { 1 } else { 0 },
    })
}

pub async fn bitmap_get_bit(
    conn: &mut RedisConnection,
    key: String,
    offset: u64,
) -> Result<bool, String> {
    validate_key(&key)?;
    let mut cmd = redis::cmd("GETBIT");
    cmd.arg(&key).arg(offset);
    let result: i64 = conn
        .query(cmd)
        .await
        .map_err(|e| format!("[REDIS_ERROR] {e}"))?;
    Ok(result != 0)
}

pub async fn bitmap_count(
    conn: &mut RedisConnection,
    key: String,
    start: Option<i64>,
    end: Option<i64>,
) -> Result<u64, String> {
    validate_key(&key)?;
    let mut cmd = redis::cmd("BITCOUNT");
    cmd.arg(&key);
    if let (Some(s), Some(e)) = (start, end) {
        cmd.arg(s).arg(e);
    }
    let count: i64 = conn
        .query(cmd)
        .await
        .map_err(|e| format!("[REDIS_ERROR] {e}"))?;
    Ok(count as u64)
}

pub async fn bitmap_pos(
    conn: &mut RedisConnection,
    key: String,
    bit: bool,
    start: Option<u64>,
    end: Option<u64>,
    count: Option<u64>,
) -> Result<Vec<u64>, String> {
    validate_key(&key)?;
    let mut cmd = redis::cmd("BITPOS");
    cmd.arg(&key).arg(if bit { 1 } else { 0 });
    if let Some(s) = start {
        cmd.arg(s);
        if let Some(e) = end {
            cmd.arg(e);
        }
    }
    if let Some(c) = count {
        cmd.arg("COUNT").arg(c);
    }
    let positions: Vec<i64> = conn
        .query(cmd)
        .await
        .map_err(|e| format!("[REDIS_ERROR] {e}"))?;
    Ok(positions.into_iter().map(|p| p as u64).collect())
}

pub async fn hll_pfadd(
    conn: &mut RedisConnection,
    key: String,
    elements: Vec<String>,
) -> Result<bool, String> {
    validate_key(&key)?;
    let mut cmd = redis::cmd("PFADD");
    cmd.arg(&key);
    for elem in &elements {
        cmd.arg(elem);
    }
    let result: i64 = conn
        .query(cmd)
        .await
        .map_err(|e| format!("[REDIS_ERROR] {e}"))?;
    Ok(result != 0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisGeoMember {
    pub member: String,
    pub longitude: f64,
    pub latitude: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisGeoPosition {
    pub longitude: f64,
    pub latitude: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisGeoSearchResult {
    pub member: String,
    pub distance: Option<f64>,
    pub hash: Option<u64>,
    pub position: Option<RedisGeoPosition>,
}

pub async fn geo_add(
    conn: &mut RedisConnection,
    key: String,
    members: Vec<RedisGeoMember>,
) -> Result<i64, String> {
    validate_key(&key)?;
    let mut cmd = redis::cmd("GEOADD");
    cmd.arg(&key);
    for m in &members {
        cmd.arg(m.longitude).arg(m.latitude).arg(&m.member);
    }
    let result: i64 = conn
        .query(cmd)
        .await
        .map_err(|e| format!("[REDIS_ERROR] {e}"))?;
    Ok(result)
}

pub async fn geo_pos(
    conn: &mut RedisConnection,
    key: String,
    members: Vec<String>,
) -> Result<Vec<Option<RedisGeoPosition>>, String> {
    validate_key(&key)?;
    let mut cmd = redis::cmd("GEOPOS");
    cmd.arg(&key);
    for m in &members {
        cmd.arg(m);
    }
    let positions: Vec<Option<(f64, f64)>> = conn
        .query(cmd)
        .await
        .map_err(|e| format!("[REDIS_ERROR] {e}"))?;
    Ok(positions
        .into_iter()
        .map(|p| {
            p.map(|(lon, lat)| RedisGeoPosition {
                longitude: lon,
                latitude: lat,
            })
        })
        .collect())
}

pub async fn geo_dist(
    conn: &mut RedisConnection,
    key: String,
    member1: String,
    member2: String,
    unit: Option<String>,
) -> Result<f64, String> {
    validate_key(&key)?;
    let mut cmd = redis::cmd("GEODIST");
    cmd.arg(&key).arg(&member1).arg(&member2);
    if let Some(u) = unit {
        cmd.arg(u);
    }
    let result: f64 = conn
        .query(cmd)
        .await
        .map_err(|e| format!("[REDIS_ERROR] {e}"))?;
    Ok(result)
}

pub async fn geo_search(
    conn: &mut RedisConnection,
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
    validate_key(&key)?;
    let mut cmd = redis::cmd("GEOSEARCH");
    cmd.arg(&key);

    if let Some(m) = member {
        cmd.arg("FROMMEMBER").arg(m);
    } else if let (Some(lon), Some(lat)) = (longitude, latitude) {
        cmd.arg("FROMLONLAT").arg(lon).arg(lat);
    } else {
        return Err(
            "[VALIDATION_ERROR] Either member or longitude+latitude is required".to_string(),
        );
    }

    cmd.arg("BYRADIUS").arg(radius).arg(&unit);

    if with_coord || with_dist || with_hash {
        cmd.arg("WITHCOORD").arg("WITHDIST").arg("WITHHASH");
    }

    if let Some(c) = count {
        cmd.arg("COUNT").arg(c);
    }

    let results: Value = conn
        .query(cmd)
        .await
        .map_err(|e| format!("[REDIS_ERROR] {e}"))?;

    let arr = match results {
        Value::Array(a) => a,
        _ => return Ok(Vec::new()),
    };

    let mut output = Vec::new();
    for item in arr {
        if let Value::Array(inner) = item {
            if inner.is_empty() {
                continue;
            }
            let member_name =
                from_redis_value::<String>(&inner[0]).map_err(|e| format!("[REDIS_ERROR] {e}"))?;
            let mut result = RedisGeoSearchResult {
                member: member_name,
                distance: None,
                hash: None,
                position: None,
            };
            if inner.len() > 1 {
                if let Ok(dist) = from_redis_value::<f64>(&inner[1]) {
                    result.distance = Some(dist);
                }
            }
            if inner.len() > 2 {
                if let Ok(hash) = from_redis_value::<u64>(&inner[2]) {
                    result.hash = Some(hash);
                }
            }
            if inner.len() > 3 {
                if let Value::Array(coord) = &inner[3] {
                    if coord.len() >= 2 {
                        if let (Ok(lon), Ok(lat)) = (
                            from_redis_value::<f64>(&coord[0]),
                            from_redis_value::<f64>(&coord[1]),
                        ) {
                            result.position = Some(RedisGeoPosition {
                                longitude: lon,
                                latitude: lat,
                            });
                        }
                    }
                }
            }
            output.push(result);
        }
    }
    Ok(output)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisRawResult {
    pub output: String,
}

fn tokenize_command(input: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    loop {
        while chars.peek().map_or(false, |c| c.is_whitespace()) {
            chars.next();
        }
        match chars.peek() {
            None => break,
            Some('"') => {
                chars.next();
                let mut tok = String::new();
                loop {
                    match chars.next() {
                        None => return Err("Unterminated double quote in command".to_string()),
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            None => return Err("Unexpected end after backslash".to_string()),
                            Some(c) => tok.push(c),
                        },
                        Some(c) => tok.push(c),
                    }
                }
                tokens.push(tok);
            }
            Some('\'') => {
                chars.next();
                let mut tok = String::new();
                loop {
                    match chars.next() {
                        None => return Err("Unterminated single quote in command".to_string()),
                        Some('\'') => break,
                        Some(c) => tok.push(c),
                    }
                }
                tokens.push(tok);
            }
            Some(_) => {
                let mut tok = String::new();
                while chars.peek().map_or(false, |c| !c.is_whitespace()) {
                    tok.push(chars.next().unwrap());
                }
                tokens.push(tok);
            }
        }
    }
    Ok(tokens)
}

fn format_redis_value(value: Value) -> String {
    match value {
        Value::Nil => "(nil)".to_string(),
        Value::Okay => "OK".to_string(),
        Value::Int(n) => format!("(integer) {n}"),
        Value::BulkString(bytes) => match String::from_utf8(bytes) {
            Ok(s) => format!("\"{s}\""),
            Err(e) => format!("(binary {} bytes)", e.into_bytes().len()),
        },
        Value::SimpleString(s) => s,
        Value::Array(items) => {
            if items.is_empty() {
                return "(empty array)".to_string();
            }
            items
                .into_iter()
                .enumerate()
                .map(|(i, v)| format!("{}) {}", i + 1, format_redis_value(v)))
                .collect::<Vec<_>>()
                .join("\n")
        }
        Value::Map(pairs) => {
            if pairs.is_empty() {
                return "(empty map)".to_string();
            }
            pairs
                .into_iter()
                .enumerate()
                .flat_map(|(i, (k, v))| {
                    [
                        format!("{}) {}", i * 2 + 1, format_redis_value(k)),
                        format!("{}) {}", i * 2 + 2, format_redis_value(v)),
                    ]
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
        Value::Set(items) => {
            if items.is_empty() {
                return "(empty set)".to_string();
            }
            items
                .into_iter()
                .enumerate()
                .map(|(i, v)| format!("{}) {}", i + 1, format_redis_value(v)))
                .collect::<Vec<_>>()
                .join("\n")
        }
        Value::Double(f) => format!("(double) {f}"),
        Value::Boolean(b) => format!("(boolean) {b}"),
        Value::VerbatimString { text, .. } => format!("\"{text}\""),
        Value::Attribute { data, .. } => format_redis_value(*data),
        Value::Push { data, .. } => {
            if data.is_empty() {
                return "(empty push)".to_string();
            }
            data.into_iter()
                .enumerate()
                .map(|(i, v)| format!("{}) {}", i + 1, format_redis_value(v)))
                .collect::<Vec<_>>()
                .join("\n")
        }
        Value::BigNumber(n) => format!("(big number) {n}"),
        Value::ServerError(e) => format!("(error) {:?}", e),
    }
}

pub async fn zrangebyscore(
    conn: &mut RedisConnection,
    key: String,
    min: String,
    max: String,
    offset: Option<u64>,
    limit: Option<u64>,
) -> Result<RedisZRangeByScoreResult, String> {
    validate_key(&key)?;

    let mut count_cmd = redis::cmd("ZCOUNT");
    count_cmd.arg(&key).arg(&min).arg(&max);
    let total: u64 = conn.query(count_cmd).await?;

    let mut cmd = redis::cmd("ZRANGEBYSCORE");
    cmd.arg(&key).arg(&min).arg(&max).arg("WITHSCORES");
    if let (Some(off), Some(lim)) = (offset, limit) {
        cmd.arg("LIMIT").arg(off).arg(lim);
    }
    let raw: Vec<String> = conn.query(cmd).await?;

    let mut members = Vec::new();
    let mut iter = raw.iter();
    while let Some(member) = iter.next() {
        if let Some(score_str) = iter.next() {
            let score: f64 = score_str
                .parse()
                .map_err(|_| format!("[REDIS_ERROR] Cannot parse score: {score_str}"))?;
            members.push(RedisZSetMember {
                member: member.clone(),
                score,
            });
        }
    }

    Ok(RedisZRangeByScoreResult { members, total })
}

pub async fn zrank(
    conn: &mut RedisConnection,
    key: String,
    member: String,
    reverse: bool,
) -> Result<Option<i64>, String> {
    validate_key(&key)?;

    let cmd_name = if reverse { "ZREVRANK" } else { "ZRANK" };
    let mut cmd = redis::cmd(cmd_name);
    cmd.arg(&key).arg(&member);
    let rank: Option<i64> = conn.query(cmd).await?;

    Ok(rank)
}

pub async fn zscore(
    conn: &mut RedisConnection,
    key: String,
    member: String,
) -> Result<Option<f64>, String> {
    validate_key(&key)?;

    let mut cmd = redis::cmd("ZSCORE");
    cmd.arg(&key).arg(&member);
    let score: Option<f64> = conn.query(cmd).await?;

    Ok(score)
}

pub async fn zmscore(
    conn: &mut RedisConnection,
    key: String,
    members: Vec<String>,
) -> Result<Vec<Option<f64>>, String> {
    validate_key(&key)?;
    if members.is_empty() {
        return Err("[VALIDATION_ERROR] At least one member is required".to_string());
    }

    let mut cmd = redis::cmd("ZMSCORE");
    cmd.arg(&key);
    for m in &members {
        cmd.arg(m);
    }
    let scores: Vec<Option<f64>> = conn.query(cmd).await?;

    Ok(scores)
}

pub async fn zrangebylex(
    conn: &mut RedisConnection,
    key: String,
    min: String,
    max: String,
    offset: Option<u64>,
    limit: Option<u64>,
) -> Result<RedisZRangeByLexResult, String> {
    validate_key(&key)?;

    let mut count_cmd = redis::cmd("ZLEXCOUNT");
    count_cmd.arg(&key).arg(&min).arg(&max);
    let total: u64 = conn.query(count_cmd).await?;

    let mut cmd = redis::cmd("ZRANGEBYLEX");
    cmd.arg(&key).arg(&min).arg(&max);
    if let (Some(off), Some(lim)) = (offset, limit) {
        cmd.arg("LIMIT").arg(off).arg(lim);
    }
    let members: Vec<String> = conn.query(cmd).await?;

    Ok(RedisZRangeByLexResult { members, total })
}

pub async fn zlexcount(
    conn: &mut RedisConnection,
    key: String,
    min: String,
    max: String,
) -> Result<u64, String> {
    validate_key(&key)?;

    let mut cmd = redis::cmd("ZLEXCOUNT");
    cmd.arg(&key).arg(&min).arg(&max);
    let count: u64 = conn.query(cmd).await?;

    Ok(count)
}

pub async fn zpopmin(
    conn: &mut RedisConnection,
    key: String,
    count: Option<u64>,
) -> Result<Vec<RedisZSetMember>, String> {
    validate_key(&key)?;

    let mut cmd = redis::cmd("ZPOPMIN");
    cmd.arg(&key);
    if let Some(c) = count {
        cmd.arg(c);
    }
    let raw: Vec<String> = conn.query(cmd).await?;

    let mut members = Vec::new();
    let mut iter = raw.iter();
    while let Some(member) = iter.next() {
        if let Some(score_str) = iter.next() {
            let score: f64 = score_str
                .parse()
                .map_err(|_| format!("[REDIS_ERROR] Cannot parse score: {score_str}"))?;
            members.push(RedisZSetMember {
                member: member.clone(),
                score,
            });
        }
    }

    Ok(members)
}

pub async fn zpopmax(
    conn: &mut RedisConnection,
    key: String,
    count: Option<u64>,
) -> Result<Vec<RedisZSetMember>, String> {
    validate_key(&key)?;

    let mut cmd = redis::cmd("ZPOPMAX");
    cmd.arg(&key);
    if let Some(c) = count {
        cmd.arg(c);
    }
    let raw: Vec<String> = conn.query(cmd).await?;

    let mut members = Vec::new();
    let mut iter = raw.iter();
    while let Some(member) = iter.next() {
        if let Some(score_str) = iter.next() {
            let score: f64 = score_str
                .parse()
                .map_err(|_| format!("[REDIS_ERROR] Cannot parse score: {score_str}"))?;
            members.push(RedisZSetMember {
                member: member.clone(),
                score,
            });
        }
    }

    Ok(members)
}

pub async fn set_operation(
    conn: &mut RedisConnection,
    keys: Vec<String>,
    op: RedisSetOperation,
) -> Result<Vec<String>, String> {
    if keys.is_empty() {
        return Err("[VALIDATION_ERROR] At least one key is required".to_string());
    }
    for k in &keys {
        validate_key(k)?;
    }

    let cmd_name = match op {
        RedisSetOperation::Inter => "SINTER",
        RedisSetOperation::Union => "SUNION",
        RedisSetOperation::Diff => "SDIFF",
    };
    let mut cmd = redis::cmd(cmd_name);
    for k in &keys {
        cmd.arg(k);
    }
    let members: Vec<String> = conn.query(cmd).await?;

    Ok(members)
}

pub async fn sismember(
    conn: &mut RedisConnection,
    key: String,
    member: String,
) -> Result<bool, String> {
    validate_key(&key)?;

    let mut cmd = redis::cmd("SISMEMBER");
    cmd.arg(&key).arg(&member);
    let exists: bool = conn.query(cmd).await?;

    Ok(exists)
}

pub async fn smove(
    conn: &mut RedisConnection,
    source: String,
    destination: String,
    member: String,
) -> Result<bool, String> {
    validate_key(&source)?;
    validate_key(&destination)?;

    let mut cmd = redis::cmd("SMOVE");
    cmd.arg(&source).arg(&destination).arg(&member);
    let moved: bool = conn.query(cmd).await?;

    Ok(moved)
}

// ── List advanced operations ────────────────────────────────────────────────

pub async fn lindex(
    conn: &mut RedisConnection,
    key: String,
    index: i64,
) -> Result<Option<String>, String> {
    validate_key(&key)?;

    let mut cmd = redis::cmd("LINDEX");
    cmd.arg(&key).arg(index);
    let value: Option<String> = conn.query(cmd).await?;

    Ok(value)
}

pub async fn lpos(
    conn: &mut RedisConnection,
    key: String,
    element: String,
    rank: Option<i64>,
    count: Option<u64>,
    maxlen: Option<u64>,
) -> Result<Vec<i64>, String> {
    validate_key(&key)?;

    let mut cmd = redis::cmd("LPOS");
    cmd.arg(&key).arg(&element);
    if let Some(r) = rank {
        cmd.arg("RANK").arg(r);
    }
    // Always send COUNT so Redis returns an array (not a bare integer).
    // Default to 1 when caller omits count.
    cmd.arg("COUNT").arg(count.unwrap_or(1));
    if let Some(ml) = maxlen {
        cmd.arg("MAXLEN").arg(ml);
    }
    let positions: Vec<i64> = conn.query(cmd).await?;

    Ok(positions)
}

pub async fn ltrim(
    conn: &mut RedisConnection,
    key: String,
    start: i64,
    stop: i64,
) -> Result<bool, String> {
    validate_key(&key)?;

    let mut cmd = redis::cmd("LTRIM");
    cmd.arg(&key).arg(start).arg(stop);
    let _: () = conn.query(cmd).await?;

    Ok(true)
}

pub async fn linsert(
    conn: &mut RedisConnection,
    key: String,
    position: RedisLInsertPosition,
    pivot: String,
    element: String,
) -> Result<i64, String> {
    validate_key(&key)?;

    let pos_str = match position {
        RedisLInsertPosition::Before => "BEFORE",
        RedisLInsertPosition::After => "AFTER",
    };
    let mut cmd = redis::cmd("LINSERT");
    cmd.arg(&key).arg(pos_str).arg(&pivot).arg(&element);
    let len: i64 = conn.query(cmd).await?;

    Ok(len)
}

pub async fn lmove(
    conn: &mut RedisConnection,
    source: String,
    destination: String,
    src_direction: RedisLMoveDirection,
    dst_direction: RedisLMoveDirection,
) -> Result<Option<String>, String> {
    validate_key(&source)?;
    validate_key(&destination)?;

    let src_dir = match src_direction {
        RedisLMoveDirection::Left => "LEFT",
        RedisLMoveDirection::Right => "RIGHT",
    };
    let dst_dir = match dst_direction {
        RedisLMoveDirection::Left => "LEFT",
        RedisLMoveDirection::Right => "RIGHT",
    };
    let mut cmd = redis::cmd("LMOVE");
    cmd.arg(&source).arg(&destination).arg(src_dir).arg(dst_dir);
    let value: Option<String> = conn.query(cmd).await?;

    Ok(value)
}

// ── Stream Consumer Group operations ────────────────────────────────────────

pub async fn xgroup_create(
    conn: &mut RedisConnection,
    key: String,
    group: String,
    start_id: String,
    mkstream: bool,
) -> Result<bool, String> {
    validate_key(&key)?;

    let mut cmd = redis::cmd("XGROUP");
    cmd.arg("CREATE").arg(&key).arg(&group).arg(&start_id);
    if mkstream {
        cmd.arg("MKSTREAM");
    }
    let result: String = conn.query(cmd).await?;
    Ok(result == "OK")
}

pub async fn xgroup_del(
    conn: &mut RedisConnection,
    key: String,
    group: String,
) -> Result<bool, String> {
    validate_key(&key)?;

    let mut cmd = redis::cmd("XGROUP");
    cmd.arg("DESTROY").arg(&key).arg(&group);
    let result: bool = conn.query(cmd).await?;
    Ok(result)
}

pub async fn xgroup_setid(
    conn: &mut RedisConnection,
    key: String,
    group: String,
    start_id: String,
) -> Result<bool, String> {
    validate_key(&key)?;

    let mut cmd = redis::cmd("XGROUP");
    cmd.arg("SETID").arg(&key).arg(&group).arg(&start_id);
    let result: String = conn.query(cmd).await?;
    Ok(result == "OK")
}

pub async fn xack(
    conn: &mut RedisConnection,
    key: String,
    group: String,
    ids: Vec<String>,
) -> Result<i64, String> {
    validate_key(&key)?;
    if ids.is_empty() {
        return Err("[VALIDATION_ERROR] At least one ID is required".to_string());
    }

    let mut cmd = redis::cmd("XACK");
    cmd.arg(&key).arg(&group);
    for id in &ids {
        cmd.arg(id);
    }
    let count: i64 = conn.query(cmd).await?;
    Ok(count)
}

pub async fn xpending(
    conn: &mut RedisConnection,
    key: String,
    group: String,
    start: Option<String>,
    end: Option<String>,
    count: Option<i64>,
    consumer: Option<String>,
) -> Result<RedisXPendingResult, String> {
    validate_key(&key)?;

    if start.is_some() && end.is_some() && count.is_some() {
        // Range mode: XPENDING key group start end count [consumer]
        let mut cmd = redis::cmd("XPENDING");
        cmd.arg(&key)
            .arg(&group)
            .arg(start.as_deref().unwrap())
            .arg(end.as_deref().unwrap())
            .arg(count.unwrap());
        if let Some(ref c) = consumer {
            cmd.arg(c);
        }
        let raw: Value = conn.query(cmd).await?;
        let entries = parse_xpending_entries(&raw)?;
        Ok(RedisXPendingResult::Entries(entries))
    } else {
        // Summary mode: XPENDING key group
        let mut cmd = redis::cmd("XPENDING");
        cmd.arg(&key).arg(&group);
        let raw: Value = conn.query(cmd).await?;
        let summary = parse_xpending_summary(&raw)?;
        Ok(RedisXPendingResult::Summary(summary))
    }
}

fn parse_xpending_summary(raw: &Value) -> Result<RedisXPendingSummary, String> {
    // XPENDING without range returns [count, min_id, max_id, [[consumer, count], ...]]
    let items = match raw {
        Value::Array(arr) if arr.len() >= 4 => arr,
        _ => return Err("[PARSE_ERROR] Unexpected XPENDING summary format".to_string()),
    };
    let count: i64 = from_redis_value(&items[0]).unwrap_or(0);
    let min_id: String = from_redis_value(&items[1]).unwrap_or_default();
    let max_id: String = from_redis_value(&items[2]).unwrap_or_default();
    let mut consumers = Vec::new();
    if let Value::Array(ref groups) = items[3] {
        for g in groups {
            if let Value::Array(ref pair) = g {
                if pair.len() >= 2 {
                    let name: String = from_redis_value(&pair[0]).unwrap_or_default();
                    let cnt: i64 = from_redis_value(&pair[1]).unwrap_or(0);
                    consumers.push((name, cnt));
                }
            }
        }
    }
    Ok(RedisXPendingSummary {
        count,
        min_id,
        max_id,
        consumers,
    })
}

fn parse_xpending_entries(raw: &Value) -> Result<Vec<RedisXPendingEntry>, String> {
    // XPENDING with range returns array of [id, consumer, idle_ms, delivery_count]
    let items = match raw {
        Value::Array(arr) => arr,
        _ => return Err("[PARSE_ERROR] Unexpected XPENDING entries format".to_string()),
    };
    let mut entries = Vec::new();
    for item in items {
        if let Value::Array(ref cols) = item {
            if cols.len() >= 4 {
                entries.push(RedisXPendingEntry {
                    id: from_redis_value(&cols[0]).unwrap_or_default(),
                    consumer: from_redis_value(&cols[1]).unwrap_or_default(),
                    idle_ms: from_redis_value(&cols[2]).unwrap_or(0),
                    delivery_count: from_redis_value(&cols[3]).unwrap_or(0),
                });
            }
        }
    }
    Ok(entries)
}

pub async fn xclaim(
    conn: &mut RedisConnection,
    key: String,
    group: String,
    consumer: String,
    min_idle_ms: i64,
    ids: Vec<String>,
) -> Result<Vec<RedisXClaimEntry>, String> {
    validate_key(&key)?;
    if ids.is_empty() {
        return Err("[VALIDATION_ERROR] At least one ID is required".to_string());
    }

    let mut cmd = redis::cmd("XCLAIM");
    cmd.arg(&key).arg(&group).arg(&consumer).arg(min_idle_ms);
    for id in &ids {
        cmd.arg(id);
    }
    let raw: Value = conn.query(cmd).await?;
    parse_xclaim_entries(&raw)
}

fn parse_xclaim_entries(raw: &Value) -> Result<Vec<RedisXClaimEntry>, String> {
    // XCLAIM returns same format as XRANGE: [[id, [field, value, ...]], ...]
    let items = match raw {
        Value::Array(arr) => arr,
        _ => return Err("[PARSE_ERROR] Unexpected XCLAIM format".to_string()),
    };
    let mut entries = Vec::new();
    for item in items {
        if let Value::Array(ref cols) = item {
            if cols.len() >= 2 {
                let id: String = from_redis_value(&cols[0]).unwrap_or_default();
                let fields = parse_stream_fields(&cols[1]);
                entries.push(RedisXClaimEntry {
                    id,
                    fields,
                    idle_ms: None,
                    delivery_count: None,
                });
            }
        }
    }
    Ok(entries)
}

fn parse_stream_fields(val: &Value) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    if let Value::Array(ref items) = val {
        let mut iter = items.iter();
        while let (Some(k), Some(v)) = (iter.next(), iter.next()) {
            if let (Ok(key), Ok(val)) =
                (from_redis_value::<String>(k), from_redis_value::<String>(v))
            {
                map.insert(key, val);
            }
        }
    }
    map
}

pub async fn xtrim(
    conn: &mut RedisConnection,
    key: String,
    strategy: String,
    threshold: String,
    approximate: Option<bool>,
) -> Result<i64, String> {
    validate_key(&key)?;

    let strategy_upper = strategy.to_uppercase();
    if strategy_upper != "MAXLEN" && strategy_upper != "MINID" {
        return Err(format!(
            "[VALIDATION_ERROR] Invalid strategy '{}': must be MAXLEN or MINID",
            strategy
        ));
    }

    let mut cmd = redis::cmd("XTRIM");
    cmd.arg(&key).arg(&strategy_upper);
    if approximate.unwrap_or(false) {
        cmd.arg("~");
    }
    cmd.arg(&threshold);
    let count: i64 = conn.query(cmd).await?;
    Ok(count)
}

pub async fn xreadgroup(
    conn: &mut RedisConnection,
    key: String,
    group: String,
    consumer: String,
    start_id: String,
    count: Option<i64>,
) -> Result<Vec<RedisStreamEntry>, String> {
    validate_key(&key)?;

    let mut cmd = redis::cmd("XREADGROUP");
    cmd.arg("GROUP").arg(&group).arg(&consumer);
    if let Some(c) = count {
        cmd.arg("COUNT").arg(c);
    }
    cmd.arg("STREAMS").arg(&key).arg(&start_id);
    let raw: Value = conn.query(cmd).await?;
    // XREADGROUP returns [[stream_name, [[id, [field, value, ...]], ...]]] or Nil
    match raw {
        Value::Nil => Ok(Vec::new()),
        Value::Array(ref streams) => {
            if let Some(Value::Array(ref stream_data)) = streams.first() {
                if stream_data.len() >= 2 {
                    return Ok(parse_xrange_value(stream_data[1].clone()));
                }
            }
            Ok(Vec::new())
        }
        _ => Ok(Vec::new()),
    }
}

// ── Raw command execution ───────────────────────────────────────────────────

pub async fn execute_raw(
    conn: &mut RedisConnection,
    command: String,
) -> Result<RedisRawResult, String> {
    let tokens = tokenize_command(&command)?;
    if tokens.is_empty() {
        return Err("[VALIDATION_ERROR] Command cannot be empty".to_string());
    }
    let mut cmd = redis::cmd(&tokens[0]);
    for arg in &tokens[1..] {
        cmd.arg(arg.as_str());
    }
    let value: Value = conn.query(cmd).await?;
    Ok(RedisRawResult {
        output: format_redis_value(value),
    })
}

// ── Batch operations ────────────────────────────────────────────────────────

pub async fn batch_key_ops(
    conn: &mut RedisConnection,
    operations: Vec<RedisBatchKeyOp>,
) -> Result<Vec<RedisBatchKeyOpResult>, String> {
    if operations.is_empty() {
        return Ok(Vec::new());
    }

    let mut results = Vec::with_capacity(operations.len());

    // Build a pipeline for the batch
    let mut pipe = redis::pipe();
    for op in &operations {
        match op.op.as_str() {
            "del" => {
                pipe.cmd("DEL").arg(&op.key);
            }
            "unlink" => {
                pipe.cmd("UNLINK").arg(&op.key);
            }
            "expire" => {
                let ttl = op.ttl_seconds.unwrap_or(0);
                pipe.cmd("EXPIRE").arg(&op.key).arg(ttl);
            }
            "persist" => {
                pipe.cmd("PERSIST").arg(&op.key);
            }
            _ => {
                return Err(format!(
                    "[VALIDATION_ERROR] Unknown batch operation: {}",
                    op.op
                ));
            }
        }
    }

    let raw_values: Vec<Value> = conn.pipe_query(&mut pipe).await?;

    for (i, val) in raw_values.into_iter().enumerate() {
        let op = &operations[i];
        let (affected, success) = match val {
            Value::Int(n) => (n, true),
            Value::Nil => (0, false),
            _ => (0, false),
        };
        results.push(RedisBatchKeyOpResult {
            key: op.key.clone(),
            op: op.op.clone(),
            success,
            affected,
        });
    }

    Ok(results)
}

pub async fn mget_keys(
    conn: &mut RedisConnection,
    keys: Vec<String>,
) -> Result<Vec<RedisMgetEntry>, String> {
    if keys.is_empty() {
        return Ok(Vec::new());
    }

    let mut cmd = redis::cmd("MGET");
    for k in &keys {
        cmd.arg(k);
    }
    let raw_values: Vec<Value> = conn.query(cmd).await?;

    let results: Vec<RedisMgetEntry> = keys
        .into_iter()
        .zip(raw_values.into_iter())
        .map(|(key, val)| match val {
            Value::BulkString(bytes) => {
                let value = String::from_utf8(bytes).unwrap_or_else(|e| {
                    // Binary data — represent as lossy UTF-8
                    String::from_utf8_lossy(e.as_bytes()).into_owned()
                });
                RedisMgetEntry {
                    key,
                    value: Some(value),
                    exists: true,
                }
            }
            Value::Nil => RedisMgetEntry {
                key,
                value: None,
                exists: false,
            },
            other => RedisMgetEntry {
                key,
                value: Some(format_redis_value(other)),
                exists: true,
            },
        })
        .collect();

    Ok(results)
}

pub async fn mset_keys(
    conn: &mut RedisConnection,
    entries: Vec<(String, String)>,
) -> Result<RedisMutationResult, String> {
    if entries.is_empty() {
        return Ok(RedisMutationResult {
            success: true,
            affected: 0,
        });
    }

    let mut cmd = redis::cmd("MSET");
    for (k, v) in &entries {
        cmd.arg(k).arg(v);
    }
    let _: String = conn.query(cmd).await?;

    Ok(RedisMutationResult {
        success: true,
        affected: entries.len() as i64,
    })
}

pub async fn cluster_info(conn: &mut RedisConnection) -> Result<RedisClusterInfo, String> {
    let mut pipe = redis::pipe();
    pipe.cmd("CLUSTER").arg("INFO");
    pipe.cmd("CLUSTER").arg("NODES");
    let (info_raw, nodes_raw): (String, String) = conn.pipe_query(&mut pipe).await?;

    let info = parse_cluster_info_text(&info_raw);
    let nodes = parse_cluster_nodes_text(&nodes_raw);

    Ok(RedisClusterInfo { info, nodes })
}

/// Parse `CLUSTER INFO` output (lines of `key:value`) into a map.
fn parse_cluster_info_text(raw: &str) -> HashMap<String, String> {
    let mut info = HashMap::new();
    for line in raw.lines() {
        if let Some((k, v)) = line.split_once(':') {
            info.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    info
}

/// Parse `CLUSTER NODES` output into a list of `RedisClusterNode`.
fn parse_cluster_nodes_text(raw: &str) -> Vec<RedisClusterNode> {
    let mut nodes = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // Format: <id> <addr> <flags> <master_id> <ping_sent> <pong_recv> <config_epoch> <link_state> <slot_range>...
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 8 {
            continue;
        }
        let flags: Vec<String> = parts[2].split(',').map(|s| s.to_string()).collect();
        let master_id = if parts[3] == "-" {
            None
        } else {
            Some(parts[3].to_string())
        };
        let slot_range = if parts.len() > 8 {
            Some(parts[8..].join(" "))
        } else {
            None
        };
        nodes.push(RedisClusterNode {
            id: parts[0].to_string(),
            addr: parts[1].to_string(),
            flags,
            master_id,
            ping_sent: parts[4].parse().unwrap_or(0),
            pong_recv: parts[5].parse().unwrap_or(0),
            config_epoch: parts[6].parse().unwrap_or(0),
            link_state: parts[7].to_string(),
            slot_range,
        });
    }
    nodes
}

#[cfg(test)]
mod tests {
    use super::{
        build_cluster_nodes, build_connection_info, is_cluster_form, list_databases,
        parse_cluster_info_text, parse_cluster_nodes_text, parse_database, redis_mode,
        validate_value_for_write, RedisValue,
    };
    use crate::models::ConnectionForm;
    use redis::ConnectionAddr;

    #[test]
    fn parse_database_accepts_db_prefix() {
        assert_eq!(parse_database(Some("db3")).unwrap(), 3);
        assert_eq!(parse_database(Some(" 4 ")).unwrap(), 4);
        assert_eq!(parse_database(None).unwrap(), 0);
    }

    #[test]
    fn parse_database_rejects_invalid_index() {
        assert!(parse_database(Some("abc")).is_err());
        assert!(parse_database(Some("256")).is_err());
    }

    #[test]
    fn redis_connection_info_preserves_acl_credentials() {
        let form = ConnectionForm {
            driver: "redis".to_string(),
            host: Some("localhost".to_string()),
            port: Some(6379),
            username: Some("app".to_string()),
            password: Some("secret".to_string()),
            ..ConnectionForm::default()
        };
        let info = build_connection_info(&form, 2).unwrap();
        assert_eq!(info.redis.db, 2);
        assert_eq!(info.redis.username.as_deref(), Some("app"));
        assert_eq!(info.redis.password.as_deref(), Some("secret"));
        assert!(matches!(info.addr, ConnectionAddr::Tcp(_, 6379)));
    }

    #[test]
    fn list_databases_marks_selected_index() {
        let form = ConnectionForm {
            driver: "redis".to_string(),
            database: Some("db5".to_string()),
            ..ConnectionForm::default()
        };
        let dbs = list_databases(&form, 16).unwrap();
        assert_eq!(dbs.len(), 16);
        assert!(dbs[5].selected);
    }

    #[test]
    fn comma_separated_hosts_enable_cluster_mode() {
        let form = ConnectionForm {
            driver: "redis".to_string(),
            host: Some("10.0.0.1:6379,10.0.0.2:6380".to_string()),
            ..ConnectionForm::default()
        };
        assert!(is_cluster_form(&form));
        assert_eq!(redis_mode(&form), "cluster");
        let nodes = build_cluster_nodes(&form).unwrap();
        assert_eq!(nodes.len(), 2);
    }

    #[test]
    fn structured_seed_nodes_enable_cluster_mode() {
        let form = ConnectionForm {
            driver: "redis".to_string(),
            mode: Some("cluster".to_string()),
            seed_nodes: Some(vec![
                "10.0.0.1:6379".to_string(),
                "10.0.0.2:6380".to_string(),
            ]),
            ..ConnectionForm::default()
        };
        assert!(is_cluster_form(&form));
        let nodes = build_cluster_nodes(&form).unwrap();
        assert_eq!(nodes.len(), 2);
    }

    #[test]
    fn cluster_mode_rejects_non_zero_database() {
        let form = ConnectionForm {
            driver: "redis".to_string(),
            host: Some("10.0.0.1:6379,10.0.0.2:6380".to_string()),
            database: Some("db1".to_string()),
            ..ConnectionForm::default()
        };
        assert!(build_cluster_nodes(&form).is_err());
    }

    #[test]
    fn password_is_optional_for_connection_info() {
        let form = ConnectionForm {
            driver: "redis".to_string(),
            host: Some("localhost".to_string()),
            port: Some(6379),
            ..ConnectionForm::default()
        };
        let info = build_connection_info(&form, 0).unwrap();
        assert!(info.redis.username.is_none());
        assert!(info.redis.password.is_none());
    }

    #[test]
    fn empty_collection_values_are_rejected_before_write() {
        assert!(validate_value_for_write(&RedisValue::Hash(Default::default())).is_err());
        assert!(validate_value_for_write(&RedisValue::List(vec![])).is_err());
        assert!(validate_value_for_write(&RedisValue::Set(vec![])).is_err());
        assert!(validate_value_for_write(&RedisValue::ZSet(vec![])).is_err());
        assert!(validate_value_for_write(&RedisValue::String(String::new())).is_ok());
    }

    use super::{format_redis_value, tokenize_command};
    use redis::Value;

    // tokenize_command

    #[test]
    fn tokenize_simple_command() {
        assert_eq!(tokenize_command("GET mykey").unwrap(), vec!["GET", "mykey"]);
    }

    #[test]
    fn tokenize_trims_extra_whitespace() {
        assert_eq!(
            tokenize_command("  SET  foo  bar  ").unwrap(),
            vec!["SET", "foo", "bar"]
        );
    }

    #[test]
    fn tokenize_double_quoted_value_with_spaces() {
        assert_eq!(
            tokenize_command(r#"SET key "hello world""#).unwrap(),
            vec!["SET", "key", "hello world"]
        );
    }

    #[test]
    fn tokenize_single_quoted_value() {
        assert_eq!(
            tokenize_command("SET key 'hello world'").unwrap(),
            vec!["SET", "key", "hello world"]
        );
    }

    #[test]
    fn tokenize_backslash_escape_in_double_quotes() {
        assert_eq!(
            tokenize_command(r#"SET key "say \"hi\"""#).unwrap(),
            vec!["SET", "key", r#"say "hi""#]
        );
    }

    #[test]
    fn tokenize_empty_string_returns_empty_vec() {
        assert_eq!(tokenize_command("").unwrap(), Vec::<String>::new());
        assert_eq!(tokenize_command("   ").unwrap(), Vec::<String>::new());
    }

    #[test]
    fn tokenize_unterminated_double_quote_is_error() {
        assert!(tokenize_command(r#"SET key "unclosed"#).is_err());
    }

    #[test]
    fn tokenize_unterminated_single_quote_is_error() {
        assert!(tokenize_command("SET key 'unclosed").is_err());
    }

    // format_redis_value

    #[test]
    fn format_nil() {
        assert_eq!(format_redis_value(Value::Nil), "(nil)");
    }

    #[test]
    fn format_okay() {
        assert_eq!(format_redis_value(Value::Okay), "OK");
    }

    #[test]
    fn format_integer() {
        assert_eq!(format_redis_value(Value::Int(42)), "(integer) 42");
        assert_eq!(format_redis_value(Value::Int(-1)), "(integer) -1");
    }

    #[test]
    fn format_bulk_string_utf8() {
        assert_eq!(
            format_redis_value(Value::BulkString(b"hello".to_vec())),
            "\"hello\""
        );
    }

    #[test]
    fn format_bulk_string_binary() {
        let bytes = vec![0xc3, 0x28]; // invalid UTF-8
        let out = format_redis_value(Value::BulkString(bytes));
        assert!(out.starts_with("(binary "));
        assert!(out.ends_with(" bytes)"));
    }

    #[test]
    fn format_simple_string() {
        assert_eq!(
            format_redis_value(Value::SimpleString("PONG".to_string())),
            "PONG"
        );
    }

    #[test]
    fn format_empty_array() {
        assert_eq!(format_redis_value(Value::Array(vec![])), "(empty array)");
    }

    #[test]
    fn format_array_with_items() {
        let items = vec![
            Value::BulkString(b"a".to_vec()),
            Value::BulkString(b"b".to_vec()),
        ];
        let out = format_redis_value(Value::Array(items));
        assert_eq!(out, "1) \"a\"\n2) \"b\"");
    }

    #[test]
    fn format_nested_array() {
        let inner = Value::Array(vec![Value::Int(1), Value::Int(2)]);
        let outer = Value::Array(vec![inner, Value::Nil]);
        let out = format_redis_value(outer);
        assert_eq!(out, "1) 1) (integer) 1\n2) (integer) 2\n2) (nil)");
    }

    // ── parse_cluster_info_text ───────────────────────────────────────────

    #[test]
    fn cluster_info_text_parses_key_value_pairs() {
        let raw = "cluster_state:ok\ncluster_slots:16384\ncluster_known_nodes:3\n";
        let info = parse_cluster_info_text(raw);
        assert_eq!(info.get("cluster_state").map(String::as_str), Some("ok"));
        assert_eq!(info.get("cluster_slots").map(String::as_str), Some("16384"));
        assert_eq!(
            info.get("cluster_known_nodes").map(String::as_str),
            Some("3")
        );
    }

    #[test]
    fn cluster_info_text_handles_empty_input() {
        let info = parse_cluster_info_text("");
        assert!(info.is_empty());
    }

    #[test]
    fn cluster_info_text_trims_whitespace() {
        let raw = "  cluster_state : ok  \n";
        let info = parse_cluster_info_text(raw);
        assert_eq!(info.get("cluster_state").map(String::as_str), Some("ok"));
    }

    // ── parse_cluster_nodes_text ──────────────────────────────────────────

    #[test]
    fn cluster_nodes_text_parses_master_and_slave() {
        let raw = "abc123 127.0.0.1:6379 myself,master - 0 1 1 connected 0-5460\ndef456 127.0.0.1:6380 slave abc123 0 2 2 connected\n";
        let nodes = parse_cluster_nodes_text(raw);
        assert_eq!(nodes.len(), 2);

        assert_eq!(nodes[0].id, "abc123");
        assert_eq!(nodes[0].addr, "127.0.0.1:6379");
        assert!(nodes[0].flags.contains(&"myself".to_string()));
        assert!(nodes[0].flags.contains(&"master".to_string()));
        assert!(nodes[0].master_id.is_none());
        assert_eq!(nodes[0].link_state, "connected");
        assert_eq!(nodes[0].slot_range.as_deref(), Some("0-5460"));

        assert_eq!(nodes[1].id, "def456");
        assert!(nodes[1].flags.contains(&"slave".to_string()));
        assert_eq!(nodes[1].master_id.as_deref(), Some("abc123"));
        assert!(nodes[1].slot_range.is_none());
    }

    #[test]
    fn cluster_nodes_text_skips_empty_lines() {
        let raw = "\n\nabc123 127.0.0.1:6379 myself,master - 0 1 1 connected 0-5460\n\n";
        let nodes = parse_cluster_nodes_text(raw);
        assert_eq!(nodes.len(), 1);
    }

    #[test]
    fn cluster_nodes_text_skips_malformed_lines() {
        let raw = "too few fields\nabc123 127.0.0.1:6379 myself,master - 0 1 1 connected 0-5460\n";
        let nodes = parse_cluster_nodes_text(raw);
        assert_eq!(nodes.len(), 1);
    }

    #[test]
    fn cluster_nodes_text_handles_empty_input() {
        let nodes = parse_cluster_nodes_text("");
        assert!(nodes.is_empty());
    }
}
