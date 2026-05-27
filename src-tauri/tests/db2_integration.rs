#[path = "common/db2_context.rs"]
mod db2_context;

use dbpaw_lib::db::drivers::DatabaseDriver;
use db2_context::get_driver;

#[tokio::test]
#[ignore]
async fn test_connection() {
    let driver = get_driver().await;
    assert!(driver.test_connection().await.is_ok());
    driver.close().await;
}

#[tokio::test]
#[ignore]
async fn test_list_databases() {
    let driver = get_driver().await;
    let dbs = driver.list_databases().await.unwrap();
    assert!(!dbs.is_empty(), "Should return at least the current database");
    println!("Databases: {:?}", dbs);
    driver.close().await;
}

#[tokio::test]
#[ignore]
async fn test_list_tables() {
    let driver = get_driver().await;
    let tables = driver.list_tables(Some("DB2INST1".to_string())).await.unwrap();
    println!("Tables: {:?}", tables);
    driver.close().await;
}

#[tokio::test]
#[ignore]
async fn test_create_table_and_get_structure() {
    let driver = get_driver().await;

    let _ = driver
        .execute_query("DROP TABLE DB2INST1.TEST_DRIVER".to_string())
        .await;

    driver
        .execute_query(
            "CREATE TABLE DB2INST1.TEST_DRIVER ( \
             ID INTEGER NOT NULL, \
             NAME VARCHAR(100), \
             AMOUNT DECIMAL(10,2), \
             PRIMARY KEY (ID) \
             )"
            .to_string(),
        )
        .await
        .unwrap();

    let structure = driver
        .get_table_structure("DB2INST1".to_string(), "TEST_DRIVER".to_string())
        .await
        .unwrap();
    assert_eq!(structure.columns.len(), 3);
    assert_eq!(structure.columns[0].name, "ID");
    assert!(structure.columns[0].primary_key);
    assert_eq!(structure.columns[1].name, "NAME");
    assert!(!structure.columns[1].primary_key);

    let _ = driver
        .execute_query("DROP TABLE DB2INST1.TEST_DRIVER".to_string())
        .await;
    driver.close().await;
}

#[tokio::test]
#[ignore]
async fn test_execute_query_select() {
    let driver = get_driver().await;
    let result = driver
        .execute_query("SELECT 1 AS NUM FROM SYSIBM.SYSDUMMY1".to_string())
        .await
        .unwrap();
    assert!(result.success);
    assert_eq!(result.row_count, 1);
    assert_eq!(result.data[0]["NUM"].as_i64().unwrap(), 1);
    driver.close().await;
}

#[tokio::test]
#[ignore]
async fn test_execute_query_dml() {
    let driver = get_driver().await;

    let _ = driver
        .execute_query("DROP TABLE DB2INST1.TEST_DML".to_string())
        .await;

    driver
        .execute_query(
            "CREATE TABLE DB2INST1.TEST_DML (ID INTEGER NOT NULL, NAME VARCHAR(50))".to_string(),
        )
        .await
        .unwrap();

    let result = driver
        .execute_query("INSERT INTO DB2INST1.TEST_DML VALUES (1, 'test')".to_string())
        .await
        .unwrap();
    assert!(result.success);

    let result = driver
        .execute_query("SELECT * FROM DB2INST1.TEST_DML".to_string())
        .await
        .unwrap();
    assert!(result.success);
    assert_eq!(result.row_count, 1);

    let _ = driver
        .execute_query("DROP TABLE DB2INST1.TEST_DML".to_string())
        .await;
    driver.close().await;
}

#[tokio::test]
#[ignore]
async fn test_get_schema_overview() {
    let driver = get_driver().await;
    let overview = driver
        .get_schema_overview(Some("DB2INST1".to_string()))
        .await
        .unwrap();
    println!("Schema overview: {} tables", overview.tables.len());
    driver.close().await;
}

#[tokio::test]
#[ignore]
async fn test_get_table_ddl() {
    let driver = get_driver().await;

    let _ = driver
        .execute_query("DROP TABLE DB2INST1.TEST_DDL".to_string())
        .await;

    driver
        .execute_query(
            "CREATE TABLE DB2INST1.TEST_DDL (ID INTEGER NOT NULL, NAME VARCHAR(50), PRIMARY KEY (ID))".to_string(),
        )
        .await
        .unwrap();

    let ddl = driver
        .get_table_ddl("DB2INST1".to_string(), "TEST_DDL".to_string())
        .await
        .unwrap();
    assert!(ddl.contains("CREATE TABLE"));
    assert!(ddl.contains("ID"));
    assert!(ddl.contains("NAME"));
    println!("Generated DDL:\n{}", ddl);

    let _ = driver
        .execute_query("DROP TABLE DB2INST1.TEST_DDL".to_string())
        .await;
    driver.close().await;
}

#[tokio::test]
#[ignore]
async fn test_list_routines() {
    let driver = get_driver().await;
    let routines = driver
        .list_routines(Some("DB2INST1".to_string()))
        .await
        .unwrap();
    println!("Routines: {:?}", routines);
    driver.close().await;
}
