use crate::models::ConnectionForm;

fn trim_string_list(values: Option<Vec<String>>) -> Option<Vec<String>> {
    values.and_then(|items| {
        let normalized = items
            .into_iter()
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .fold(Vec::<String>::new(), |mut acc, item| {
                if !acc.iter().any(|existing| existing == &item) {
                    acc.push(item);
                }
                acc
            });
        if normalized.is_empty() {
            None
        } else {
            Some(normalized)
        }
    })
}

fn trim_to_option(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .and_then(|v| if v.is_empty() { None } else { Some(v) })
}

fn trim_preserve_empty(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_string())
}

fn parse_host_embedded_port(host: &str, fallback_port: Option<i64>) -> (String, Option<i64>) {
    if host.starts_with('[') || host.contains(' ') || host.matches(':').count() != 1 {
        return (host.to_string(), fallback_port);
    }
    let Some((host_part, port_part)) = host.rsplit_once(':') else {
        return (host.to_string(), fallback_port);
    };
    if host_part.is_empty() || !port_part.chars().all(|c| c.is_ascii_digit()) {
        return (host.to_string(), fallback_port);
    }
    let parsed_port = port_part.parse::<i64>().ok();
    (host_part.to_string(), parsed_port)
}

fn validate_port_range(field: &str, port: Option<i64>) -> Result<(), String> {
    if let Some(v) = port {
        if !(1..=65535).contains(&v) {
            return Err(format!(
                "[VALIDATION_ERROR] {} must be between 1 and 65535",
                field
            ));
        }
    }
    Ok(())
}

fn normalize_redis_options(form: &mut ConnectionForm) -> Result<(), String> {
    let mode = form
        .mode
        .as_deref()
        .map(|value| value.trim().to_ascii_lowercase());
    form.mode = match mode.as_deref() {
        Some("standalone") | Some("cluster") | Some("sentinel") => mode,
        _ => None,
    };
    form.seed_nodes = trim_string_list(form.seed_nodes.take());
    form.sentinels = trim_string_list(form.sentinels.take());

    if let Some(timeout_ms) = form.connect_timeout_ms {
        if timeout_ms <= 0 {
            return Err("[VALIDATION_ERROR] connect timeout must be greater than 0".to_string());
        }
    }

    if let Some(host) = form.host.clone() {
        let detected_mode = if form.mode.is_some() {
            form.mode.clone()
        } else if host
            .split(',')
            .filter(|part| !part.trim().is_empty())
            .count()
            > 1
        {
            Some("cluster".to_string())
        } else {
            Some("standalone".to_string())
        };
        form.mode = detected_mode;
    } else if form.mode.is_none() {
        form.mode = Some("standalone".to_string());
    }

    match form.mode.as_deref() {
        Some("standalone") => {
            if let Some(host) = form.host.clone() {
                let seed = if let Some(port) = form.port {
                    format!("{host}:{port}")
                } else {
                    host
                };
                form.seed_nodes = trim_string_list(Some(vec![seed]));
            }
        }
        Some("cluster") => {
            if form.seed_nodes.is_none() {
                if let Some(host) = form.host.clone() {
                    form.seed_nodes = trim_string_list(Some(
                        host.split(',').map(|part| part.to_string()).collect(),
                    ));
                }
            }
            if form
                .seed_nodes
                .as_ref()
                .map(|nodes| nodes.len())
                .unwrap_or(0)
                < 2
            {
                return Err(
                    "[VALIDATION_ERROR] Redis cluster requires at least two seed nodes".to_string(),
                );
            }
        }
        Some("sentinel") => {
            if form
                .sentinels
                .as_ref()
                .map(|nodes| nodes.is_empty())
                .unwrap_or(true)
            {
                return Err(
                    "[VALIDATION_ERROR] Redis sentinel requires at least one sentinel node"
                        .to_string(),
                );
            }
            if form.service_name.is_none() {
                form.service_name = Some("mymaster".to_string());
            }
        }
        _ => {}
    }

    Ok(())
}

pub fn normalize_connection_form(mut form: ConnectionForm) -> Result<ConnectionForm, String> {
    form.name = trim_to_option(form.name);
    form.host = trim_to_option(form.host);
    form.database = trim_to_option(form.database);
    form.schema = trim_to_option(form.schema);
    form.username = trim_to_option(form.username);
    form.password = trim_preserve_empty(form.password);
    form.ssl_ca_cert = trim_preserve_empty(form.ssl_ca_cert);
    form.file_path = trim_to_option(form.file_path);
    form.ssh_host = trim_to_option(form.ssh_host);
    form.ssh_username = trim_to_option(form.ssh_username);
    form.ssh_password = trim_preserve_empty(form.ssh_password);
    form.ssh_key_path = trim_to_option(form.ssh_key_path);
    form.mode = trim_to_option(form.mode);
    form.seed_nodes = trim_string_list(form.seed_nodes);
    form.sentinels = trim_string_list(form.sentinels);
    form.auth_mode = trim_to_option(form.auth_mode)
        .map(|value| value.to_ascii_lowercase())
        .filter(|value| matches!(value.as_str(), "none" | "basic" | "api_key"));
    form.api_key_id = trim_to_option(form.api_key_id);
    form.api_key_secret = trim_preserve_empty(form.api_key_secret);
    form.api_key_encoded = trim_preserve_empty(form.api_key_encoded);
    form.cloud_id = trim_to_option(form.cloud_id);
    form.service_name = trim_to_option(form.service_name);
    form.sentinel_password = trim_preserve_empty(form.sentinel_password);
    form.auth_source = trim_to_option(form.auth_source);

    validate_port_range("port", form.port)?;
    validate_port_range("ssh port", form.ssh_port)?;

    let driver = form.driver.to_ascii_lowercase();
    form.driver = driver.clone();
    if crate::db::drivers::is_mysql_family_driver(&driver) || driver == "elasticsearch" || driver == "mongodb" {
        if let Some(host) = form.host.clone() {
            let (normalized_host, normalized_port) = parse_host_embedded_port(&host, form.port);
            form.host = Some(normalized_host);
            form.port = normalized_port.or(form.port);
        }
    }

    if driver == "redis" {
        if let Some(host) = form.host.clone() {
            let should_parse_host =
                form.mode.as_deref().unwrap_or("standalone") == "standalone" && !host.contains(',');
            if should_parse_host {
                let (normalized_host, normalized_port) = parse_host_embedded_port(&host, form.port);
                form.host = Some(normalized_host);
                form.port = normalized_port.or(form.port);
            }
        }
        normalize_redis_options(&mut form)?;
    }

    if matches!(driver.as_str(), "sqlite" | "duckdb") {
        if form.file_path.is_none() {
            return Err("[VALIDATION_ERROR] file path cannot be empty".to_string());
        }
    } else if driver == "redis" {
        if form.mode.as_deref() == Some("standalone") && form.host.is_none() {
            return Err("[VALIDATION_ERROR] host cannot be empty".to_string());
        }
    } else if driver == "elasticsearch" {
        if form.host.is_none() && form.cloud_id.is_none() {
            return Err("[VALIDATION_ERROR] host or cloudId cannot be empty".to_string());
        }
    } else if driver == "mongodb" {
        if form.host.is_none() {
            return Err("[VALIDATION_ERROR] host cannot be empty".to_string());
        }
    } else if form.host.is_none() {
        return Err("[VALIDATION_ERROR] host cannot be empty".to_string());
    }

    if form.ssh_enabled.unwrap_or(false) {
        if form.ssh_host.is_none() {
            return Err("[VALIDATION_ERROR] ssh host cannot be empty".to_string());
        }
        if form.ssh_username.is_none() {
            return Err("[VALIDATION_ERROR] ssh username cannot be empty".to_string());
        }
        if form.ssh_port.is_none() {
            form.ssh_port = Some(22);
        }
        if form.ssh_password.is_none() && form.ssh_key_path.is_none() {
            return Err("[VALIDATION_ERROR] ssh password or ssh key path is required".to_string());
        }
    }

    Ok(form)
}

#[cfg(test)]
mod tests {
    use super::normalize_connection_form;
    use crate::models::ConnectionForm;

    #[test]
    fn normalize_trims_fields_and_parses_mysql_host_port() {
        let form = ConnectionForm {
            driver: "starrocks".to_string(),
            host: Some(" 127.0.0.1:3307 ".to_string()),
            port: None,
            username: Some(" root ".to_string()),
            password: Some(" pass ".to_string()),
            ..Default::default()
        };
        let normalized = normalize_connection_form(form).unwrap();
        assert_eq!(normalized.host, Some("127.0.0.1".to_string()));
        assert_eq!(normalized.port, Some(3307));
        assert_eq!(normalized.username, Some("root".to_string()));
    }

    #[test]
    fn normalize_prefers_embedded_starrocks_port_over_existing_port() {
        let form = ConnectionForm {
            driver: "starrocks".to_string(),
            host: Some("127.0.0.1:9031".to_string()),
            port: Some(9030),
            username: Some("root".to_string()),
            ..Default::default()
        };

        let normalized = normalize_connection_form(form).unwrap();
        assert_eq!(normalized.host, Some("127.0.0.1".to_string()));
        assert_eq!(normalized.port, Some(9031));
    }

    #[test]
    fn normalize_prefers_embedded_doris_port_over_existing_port() {
        let form = ConnectionForm {
            driver: "doris".to_string(),
            host: Some("127.0.0.1:9031".to_string()),
            port: Some(9030),
            username: Some("root".to_string()),
            ..Default::default()
        };

        let normalized = normalize_connection_form(form).unwrap();
        assert_eq!(normalized.host, Some("127.0.0.1".to_string()));
        assert_eq!(normalized.port, Some(9031));
    }

    #[test]
    fn normalize_prefers_embedded_mysql_port_over_existing_port() {
        let form = ConnectionForm {
            driver: "mysql".to_string(),
            host: Some("127.0.0.1:3307".to_string()),
            port: Some(3306),
            username: Some("root".to_string()),
            ..Default::default()
        };

        let normalized = normalize_connection_form(form).unwrap();
        assert_eq!(normalized.host, Some("127.0.0.1".to_string()));
        assert_eq!(normalized.port, Some(3307));
    }

    #[test]
    fn normalize_preserves_empty_secret_fields_when_present() {
        let form = ConnectionForm {
            driver: "mysql".to_string(),
            host: Some(" localhost ".to_string()),
            username: Some(" root ".to_string()),
            password: Some("   ".to_string()),
            ssl_ca_cert: Some("   ".to_string()),
            ssh_password: Some("   ".to_string()),
            ..Default::default()
        };

        let normalized = normalize_connection_form(form).unwrap();
        assert_eq!(normalized.password, Some(String::new()));
        assert_eq!(normalized.ssl_ca_cert, Some(String::new()));
        assert_eq!(normalized.ssh_password, Some(String::new()));
    }

    #[test]
    fn normalize_rejects_out_of_range_ports() {
        let form = ConnectionForm {
            driver: "postgres".to_string(),
            host: Some("localhost".to_string()),
            port: Some(70000),
            username: Some("postgres".to_string()),
            ..Default::default()
        };
        assert!(normalize_connection_form(form).is_err());
    }

    #[test]
    fn normalize_redis_cluster_seed_nodes_and_mode() {
        let form = ConnectionForm {
            driver: "redis".to_string(),
            mode: Some("cluster".to_string()),
            seed_nodes: Some(vec![
                " 10.0.0.1:6379 ".to_string(),
                "10.0.0.2:6379".to_string(),
            ]),
            ..Default::default()
        };
        let normalized = normalize_connection_form(form).unwrap();
        assert_eq!(normalized.mode.as_deref(), Some("cluster"));
        assert_eq!(
            normalized.seed_nodes.unwrap(),
            vec!["10.0.0.1:6379".to_string(), "10.0.0.2:6379".to_string()]
        );
    }

    #[test]
    fn normalize_redis_legacy_cluster_host_into_seed_nodes() {
        let form = ConnectionForm {
            driver: "redis".to_string(),
            host: Some("10.0.0.1:6379,10.0.0.2:6379".to_string()),
            ..Default::default()
        };
        let normalized = normalize_connection_form(form).unwrap();
        assert_eq!(normalized.mode.as_deref(), Some("cluster"));
        assert_eq!(
            normalized.seed_nodes.unwrap(),
            vec!["10.0.0.1:6379".to_string(), "10.0.0.2:6379".to_string()]
        );
    }
}
