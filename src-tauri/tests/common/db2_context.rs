mod shared;

use dbpaw_lib::db::drivers::db2::Db2Driver;
use dbpaw_lib::models::ConnectionForm;

#[allow(unused_imports)]
pub use shared::{connect_with_retry, should_reuse_local_db};

pub fn db2_connection_form() -> ConnectionForm {
    ConnectionForm {
        driver: "db2".to_string(),
        host: Some(shared::env_or("DB2_HOST", "127.0.0.1")),
        port: Some(shared::env_i64("DB2_PORT", 50000)),
        database: Some(shared::env_or("DB2_DATABASE", "testdb")),
        username: Some(shared::env_or("DB2_USERNAME", "db2inst1")),
        password: Some(shared::env_or("DB2_PASSWORD", "testpass")),
        ..Default::default()
    }
}

pub async fn get_driver() -> Db2Driver {
    let form = db2_connection_form();
    connect_with_retry(|| Db2Driver::connect(&form)).await
}
