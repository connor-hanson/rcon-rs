use std::fs::File;
use std::collections::HashMap;

use serde::Deserialize;
use serde_json;

#[derive(Deserialize, Clone, Debug)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub password: String,
}

#[derive(Deserialize, Debug)]
struct ServerConfigMap {
    configs: HashMap<String, ServerConfig>,
}

const ENV_VAR_KEY: &str = "RCON_CONFIG_PATH";

fn get_config_path_env_var() -> Option<String> {
    let env_var = std::env::var(ENV_VAR_KEY);
    match env_var {
        Ok(path) => {
            log::debug!("Found environment variable {}: {}", ENV_VAR_KEY, path);
            Some(path)
        },
        Err(_) => {
            log::warn!("Environment variable {} not set", ENV_VAR_KEY);
            None
        }
    }
}

pub fn load_config_from_env(config_name: Option<String>) -> Option<ServerConfig> {
    if let Some(config_path) = get_config_path_env_var() {
        load_config(&config_path, config_name)
    } else {
        None
    }
}

fn load_config(config_file_path: &str, config_name: Option<String>) -> Option<ServerConfig> {
    let mut file = match File::open(config_file_path) {
        Ok(f) => f,
        Err(_) => {
            log::error!("Failed to open config file: {}", config_file_path);
            return None;
        }
    };

    let config: Result<ServerConfigMap, serde_json::Error> = serde_json::from_reader(&mut file);
    let config = match config {
        Ok(c) => c,
        Err(_) => {
            log::error!("Failed to parse config file: {}", config_file_path);
            return None;
        }
    };
    log::debug!("Loaded config file: {:?}", config); 

    if let Some(name) = config_name {
        if let Some(server_config) = config.configs.get(&name) {
            log::info!("Using config: {}", name);
            log::debug!("Server config: {:?}", server_config);
            return Some(server_config.clone());
        } else {
            log::error!("Config with name '{}' not found in config file.", name);
            return None;
        }
    } else {
        if config.configs.len() == 1 {
            let (name, server_config) = config.configs.iter().next().unwrap();
            log::info!("No config name provided. Using the only available config: {}", name);
            log::debug!("Server config: {:?}", server_config);
            return Some(server_config.clone());
        } else {
            log::error!("No config name provided. Please specify a config name.");
            return None;
        }   
    }
}

#[cfg(test)]
mod tests {
    use super::{load_config, ServerConfig};
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_config_file(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("Failed to create temp file");
        file.write_all(content.as_bytes())
            .expect("Failed to write to temp file");
        file.flush().expect("Failed to flush temp file");
        file
    }

    #[test]
    fn test_load_config_with_specific_name() {
        let config_content = r#"{
            "configs": {
                "server1": {
                    "host": "192.168.1.1",
                    "port": 27015,
                    "password": "password123"
                },
                "server2": {
                    "host": "192.168.1.2",
                    "port": 27016,
                    "password": "password456"
                }
            }
        }"#;

        let temp_file = create_test_config_file(config_content);
        let result = load_config(temp_file.path().to_str().unwrap(), Some("server1".to_string()));

        assert!(result.is_some());
        let config = result.unwrap();
        assert_eq!(config.host, "192.168.1.1");
        assert_eq!(config.port, 27015);
        assert_eq!(config.password, "password123");
    }

    #[test]
    fn test_load_config_with_single_config_no_name() {
        let config_content = r#"{
            "configs": {
                "default": {
                    "host": "localhost",
                    "port": 27575,
                    "password": "admin"
                }
            }
        }"#;

        let temp_file = create_test_config_file(config_content);
        let result = load_config(temp_file.path().to_str().unwrap(), None);

        assert!(result.is_some());
        let config = result.unwrap();
        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 27575);
        assert_eq!(config.password, "admin");
    }

    #[test]
    fn test_load_config_nonexistent_name() {
        let config_content = r#"{
            "configs": {
                "server1": {
                    "host": "192.168.1.1",
                    "port": 27015,
                    "password": "password123"
                }
            }
        }"#;

        let temp_file = create_test_config_file(config_content);
        let result = load_config(
            temp_file.path().to_str().unwrap(),
            Some("nonexistent".to_string()),
        );

        assert!(result.is_none());
    }

    #[test]
    fn test_load_config_multiple_servers_no_name() {
        let config_content = r#"{
            "configs": {
                "server1": {
                    "host": "192.168.1.1",
                    "port": 27015,
                    "password": "password123"
                },
                "server2": {
                    "host": "192.168.1.2",
                    "port": 27016,
                    "password": "password456"
                }
            }
        }"#;

        let temp_file = create_test_config_file(config_content);
        let result = load_config(temp_file.path().to_str().unwrap(), None);

        // Should return None when multiple configs exist and no name is provided
        assert!(result.is_none());
    }

    #[test]
    fn test_load_config_invalid_json() {
        let config_content = r#"{ invalid json }"#;

        let temp_file = create_test_config_file(config_content);
        let result = load_config(temp_file.path().to_str().unwrap(), None);

        assert!(result.is_none());
    }

    #[test]
    fn test_load_config_nonexistent_file() {
        let result = load_config("/nonexistent/path/to/config.json", None);

        assert!(result.is_none());
    }

    #[test]
    fn test_load_config_empty_configs() {
        let config_content = r#"{
            "configs": {}
        }"#;

        let temp_file = create_test_config_file(config_content);
        let result = load_config(temp_file.path().to_str().unwrap(), None);

        assert!(result.is_none());
    }

    #[test]
    fn test_server_config_clone() {
        let config = ServerConfig {
            host: "example.com".to_string(),
            port: 8080,
            password: "secret".to_string(),
        };

        let cloned = config.clone();
        assert_eq!(config.host, cloned.host);
        assert_eq!(config.password, cloned.password);
    }
}