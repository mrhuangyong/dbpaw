mod shared;

use dbpaw_lib::models::ConnectionForm;
use std::time::Duration;
use testcontainers::clients::Cli;
use testcontainers::core::WaitFor;
use testcontainers::{Container, GenericImage, RunnableImage};

pub use shared::should_reuse_local_db;

pub fn mongodb_form_from_test_context<'a>(
    docker: Option<&'a Cli>,
) -> (Option<Container<'a, GenericImage>>, ConnectionForm) {
    if should_reuse_local_db() {
        return (None, mongodb_form_from_local_env());
    }
    shared::ensure_docker_available();

    let docker = docker.expect("docker client is required when IT_REUSE_LOCAL_DB is not enabled");
    let image = GenericImage::new("mongo", "7.0")
        .with_wait_for(WaitFor::seconds(15))
        .with_exposed_port(27017);
    let runnable = RunnableImage::from(image)
        .with_container_name(shared::unique_container_name("mongodb"));
    let container = docker.run(runnable);
    let port = container.get_host_port_ipv4(27017);
    shared::wait_for_port("127.0.0.1", port, Duration::from_secs(60));

    (
        Some(container),
        ConnectionForm {
            driver: "mongodb".to_string(),
            host: Some("127.0.0.1".to_string()),
            port: Some(i64::from(port)),
            ..Default::default()
        },
    )
}

fn mongodb_form_from_local_env() -> ConnectionForm {
    ConnectionForm {
        driver: "mongodb".to_string(),
        host: Some(shared::env_or("MONGODB_HOST", "127.0.0.1")),
        port: Some(shared::env_i64("MONGODB_PORT", 27017)),
        username: std::env::var("MONGODB_USER").ok(),
        password: std::env::var("MONGODB_PASSWORD").ok(),
        auth_source: std::env::var("MONGODB_AUTH_SOURCE").ok(),
        ssl: Some(
            std::env::var("MONGODB_SSL")
                .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
        ),
        ..Default::default()
    }
}
