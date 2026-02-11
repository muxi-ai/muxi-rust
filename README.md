# MUXI Rust SDK

Official Rust SDK for [MUXI](https://muxi.ai) — infrastructure for AI agents.

**Highlights**
- Async/await with `tokio` and `reqwest`
- Built-in retries, idempotency, and typed errors
- Streaming helpers for chat/audio and deploy/log tails

> Need deeper usage notes? See the [User Guide](https://github.com/muxi-ai/muxi-rust/blob/main/USER_GUIDE.md) for streaming, retries, and auth details.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
muxi = "0.20260211.0"
tokio = { version = "1", features = ["full"] }
```

## Quick Start

### Server Management (Control Plane)

```rust
use muxi::{ServerClient, ServerConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = ServerClient::new(ServerConfig {
        url: std::env::var("MUXI_SERVER_URL")?,
        key_id: std::env::var("MUXI_KEY_ID")?,
        secret_key: std::env::var("MUXI_SECRET_KEY")?,
        ..Default::default()
    });

    // List formations
    let formations = server.list_formations().await?;
    println!("{:?}", formations);

    // Get server status
    let status = server.status().await?;
    println!("Status: {:?}", status);

    Ok(())
}
```

### Formation Usage (Runtime API)

```rust
use muxi::{FormationClient, FormationConfig};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect via server proxy
    let client = FormationClient::new(FormationConfig {
        formation_id: Some("my-bot".to_string()),
        server_url: Some(std::env::var("MUXI_SERVER_URL")?),
        client_key: Some(std::env::var("MUXI_CLIENT_KEY")?),
        admin_key: Some(std::env::var("MUXI_ADMIN_KEY")?),
        ..Default::default()
    });

    // Or connect directly to formation
    let client = FormationClient::new(FormationConfig {
        url: Some("http://localhost:8001".to_string()),
        client_key: Some(std::env::var("MUXI_CLIENT_KEY")?),
        admin_key: Some(std::env::var("MUXI_ADMIN_KEY")?),
        ..Default::default()
    });

    // Chat (non-streaming)
    let response = client.chat(json!({"message": "Hello!"}), "user123").await?;
    println!("{:?}", response);

    // Health check
    let health = client.health().await?;
    println!("Status: {:?}", health);

    Ok(())
}
```

## Auth & Headers

- **Server**: HMAC with `key_id`/`secret_key` on `/rpc/*` endpoints
- **Formation**: `X-MUXI-CLIENT-KEY` or `X-MUXI-ADMIN-KEY` headers
- **Idempotency**: `X-Muxi-Idempotency-Key` auto-generated on every request
- **SDK**: `X-Muxi-SDK`, `X-Muxi-Client` headers set automatically

## Error Handling

```rust
use muxi::errors::*;

match client.chat(payload, user_id).await {
    Ok(response) => println!("{:?}", response),
    Err(MuxiError::NotFound { message, .. }) => {
        println!("Not found: {}", message);
    }
    Err(MuxiError::Authentication { message, .. }) => {
        println!("Auth failed: {}", message);
    }
    Err(MuxiError::RateLimit { retry_after, .. }) => {
        println!("Rate limited. Retry after: {:?}s", retry_after);
    }
    Err(e) => println!("Error: {}", e),
}
```

## Configuration

```rust
let server = ServerClient::new(ServerConfig {
    url: "https://muxi.example.com:7890".to_string(),
    key_id: "your-key-id".to_string(),
    secret_key: "your-secret-key".to_string(),
    timeout: Some(30),      // Request timeout in seconds
    max_retries: Some(3),   // Retry on 429/5xx errors
    debug: Some(true),      // Enable debug logging
    ..Default::default()
});
```

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.

## Links

- [MUXI SDKs](https://github.com/muxi-ai/sdks)
- [MUXI Documentation](https://docs.muxi.ai)
- [GitHub](https://github.com/muxi-ai/muxi-rust)
