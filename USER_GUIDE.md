# MUXI Rust SDK User Guide

## Installation

```toml
[dependencies]
muxi = "0.20260211.0"
tokio = { version = "1", features = ["full"] }
```

## Clients

- **ServerClient** (management, HMAC): deploy/list/update formations, server health/status, logs
- **FormationClient** (runtime, client/admin keys): chat/audio, agents, secrets, MCP, memory, scheduler, sessions, triggers, etc.

## Quick Start

### ServerClient

```rust
use muxi_rust::{ServerClient, ServerConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = ServerClient::new(ServerConfig {
        url: std::env::var("MUXI_SERVER_URL")?,
        key_id: std::env::var("MUXI_KEY_ID")?,
        secret_key: std::env::var("MUXI_SECRET_KEY")?,
        ..Default::default()
    });

    let status = server.status().await?;
    println!("{:?}", status);

    Ok(())
}
```

### FormationClient

```rust
use muxi_rust::{FormationClient, FormationConfig};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = FormationClient::new(FormationConfig {
        formation_id: Some("my-bot".to_string()),
        server_url: Some(std::env::var("MUXI_SERVER_URL")?),
        client_key: Some(std::env::var("MUXI_CLIENT_KEY")?),
        admin_key: Some(std::env::var("MUXI_ADMIN_KEY")?),
        ..Default::default()
    });

    let response = client.chat(json!({"message": "Hello"}), "user123").await?;
    println!("{:?}", response);

    Ok(())
}
```

## Auth & Headers

- **ServerClient**: HMAC signature (`MUXI-HMAC key=<id>, timestamp=<sec>, signature=<b64>`)
- **FormationClient**: `X-MUXI-CLIENT-KEY` required; `X-MUXI-ADMIN-KEY` for admin endpoints
- **Idempotency**: `X-Muxi-Idempotency-Key` auto-generated on every request
- **SDK Headers**: `X-Muxi-SDK: rust/{version}`, `X-Muxi-Client: {os}/{arch}`

## Timeouts & Retries

- Default timeout: 30s (configurable)
- Retries on 429/5xx with exponential backoff
- Respects `Retry-After` header for rate limits

## Error Handling

```rust
use muxi_rust::errors::*;

match result {
    Err(MuxiError::Authentication { code, message, .. }) => { /* 401 */ }
    Err(MuxiError::Authorization { code, message, .. }) => { /* 403 */ }
    Err(MuxiError::NotFound { code, message, .. }) => { /* 404 */ }
    Err(MuxiError::Validation { code, message, .. }) => { /* 422 */ }
    Err(MuxiError::RateLimit { message, retry_after, .. }) => { /* 429 */ }
    Err(MuxiError::Server { code, message, .. }) => { /* 5xx */ }
    Err(MuxiError::Connection { message }) => { /* network error */ }
    _ => {}
}
```

## Webhook Verification

```rust
use muxi_rust::webhook::{verify_signature, parse};

fn handle_webhook(payload: &str, signature: &str, secret: &str) -> Result<(), Box<dyn std::error::Error>> {
    if !verify_signature(payload, signature, secret, None)? {
        return Err("Invalid signature".into());
    }

    let event = parse(payload)?;
    
    match event.status.as_str() {
        "completed" => {
            for item in &event.content {
                if item.item_type == "text" {
                    println!("{}", item.text.as_deref().unwrap_or(""));
                }
            }
        }
        "failed" => println!("Error: {:?}", event.error),
        "awaiting_clarification" => println!("Question: {:?}", event.clarification),
        _ => {}
    }

    Ok(())
}
```

## Testing

```bash
cargo test
```

## Contributing

- Format with `cargo fmt` before commit
- Run `cargo clippy` for lints
- Preserve idempotency header injection
