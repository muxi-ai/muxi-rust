//! Integration tests for MUXI Rust SDK

use std::env;

fn env_var(name: &str) -> Option<String> {
    env::var(name).ok().filter(|s| !s.is_empty())
}

fn require_env(name: &str) -> String {
    env_var(name).unwrap_or_else(|| {
        eprintln!("Skipping: {} not set", name);
        String::new()
    })
}

fn skip_if_not_configured() -> bool {
    let required = [
        "MUXI_SDK_E2E_SERVER_URL",
        "MUXI_SDK_E2E_KEY_ID", 
        "MUXI_SDK_E2E_SECRET_KEY",
        "MUXI_SDK_E2E_FORMATION_ID",
        "MUXI_SDK_E2E_CLIENT_KEY",
        "MUXI_SDK_E2E_ADMIN_KEY",
    ];
    required.iter().any(|name| env_var(name).is_none())
}

#[cfg(test)]
mod server_tests {
    use super::*;
    use muxi_rust::{ServerClient, ServerConfig};

    fn get_client() -> Option<ServerClient> {
        if skip_if_not_configured() {
            return None;
        }
        let config = ServerConfig::new(
            &require_env("MUXI_SDK_E2E_SERVER_URL"),
            &require_env("MUXI_SDK_E2E_KEY_ID"),
            &require_env("MUXI_SDK_E2E_SECRET_KEY"),
        );
        ServerClient::new(config).ok()
    }

    #[tokio::test]
    async fn test_health() {
        let Some(client) = get_client() else { return };
        let result = client.health().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_status() {
        let Some(client) = get_client() else { return };
        let result = client.status().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_formations() {
        let Some(client) = get_client() else { return };
        let result = client.list_formations().await;
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod formation_tests {
    use super::*;
    use muxi_rust::{FormationClient, FormationConfig};

    fn get_client() -> Option<FormationClient> {
        if skip_if_not_configured() {
            return None;
        }
        let config = FormationConfig::new(
            &require_env("MUXI_SDK_E2E_SERVER_URL"),
            &require_env("MUXI_SDK_E2E_FORMATION_ID"),
            &require_env("MUXI_SDK_E2E_CLIENT_KEY"),
            &require_env("MUXI_SDK_E2E_ADMIN_KEY"),
        );
        FormationClient::new(config).ok()
    }

    #[tokio::test]
    async fn test_health() {
        let Some(client) = get_client() else { return };
        let result = client.health().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_status() {
        let Some(client) = get_client() else { return };
        let result = client.get_status().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_config() {
        let Some(client) = get_client() else { return };
        let result = client.get_config().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_agents() {
        let Some(client) = get_client() else { return };
        let result = client.get_agents().await;
        assert!(result.is_ok());
    }
}
