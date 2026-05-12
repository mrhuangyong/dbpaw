#[path = "common/mongodb_context.rs"]
mod mongodb_context;

use dbpaw_lib::datasources::mongodb::MongodbClient;
use testcontainers::clients::Cli;

#[tokio::test]
#[ignore]
async fn test_mongodb_connection_and_list_databases() {
    let docker = (!mongodb_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) =
        mongodb_context::mongodb_form_from_test_context(docker.as_ref());
    let client = MongodbClient::connect(&form).await.expect("connect client");

    let info = client.test_connection().await.expect("test_connection");
    assert!(
        info.version.is_some(),
        "Expected MongoDB version in connection info"
    );

    let databases = client.list_databases().await.expect("list_databases");
    assert!(
        !databases.is_empty(),
        "Expected at least one database (admin)"
    );

    let names: Vec<&str> = databases.iter().map(|db| db.name.as_str()).collect();
    assert!(
        names.contains(&"admin"),
        "Expected 'admin' database in list"
    );
}

#[tokio::test]
#[ignore]
async fn test_mongodb_list_collections() {
    let docker = (!mongodb_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) =
        mongodb_context::mongodb_form_from_test_context(docker.as_ref());
    let client = MongodbClient::connect(&form).await.expect("connect client");

    let collections = client
        .list_collections("admin")
        .await
        .expect("list_collections on admin db");
    // admin db may or may not have user collections, but should not error
    let _ = collections;

    let collections = client
        .list_collections("local")
        .await
        .expect("list_collections on local db");
    let _ = collections;
}
