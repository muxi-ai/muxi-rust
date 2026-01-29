# MUXI Rust SDK User Guide

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
muxi = "0.1"
tokio = { version = "1", features = ["full"] }
```

## Quickstart

```rust
use muxi::{ServerClient, FormationClient};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Server client (management, HMAC auth)
    let server = ServerClient::new(
        "https://server.example.com",
        "<key_id>",
        "<secret_key>",
    );
    println!("{:?}", server.status().await?);

    // Formation client (runtime, key auth)
    let formation = FormationClient::new(
        "https://server.example.com",
        "<formation_id>",
        "<client_key>",
        Some("<admin_key>"),
    );
    println!("{:?}", formation.health().await?);

    Ok(())
}
```

## Clients

- **ServerClient** (management, HMAC): deploy/list/update formations, server health/status, server logs.
- **FormationClient** (runtime, client/admin keys): chat/audio (streaming), agents, secrets, MCP, memory, scheduler, sessions/requests, identifiers, credentials, triggers/SOPs/audit, async/A2A/logging config, overlord/LLM settings, events/logs streaming.

## Streaming

```rust
use futures::StreamExt;
use serde_json::json;

// Chat streaming
let request = json!({ "message": "Tell me a story" });
let mut stream = formation.chat_stream(request, "user-123").await?;

while let Some(event) = stream.next().await {
    match event {
        Ok(sse) => {
            if sse.event == Some("message".to_string()) {
                println!("{}", sse.data);
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}

// Event streaming
let mut events = formation.stream_events("user-123").await?;
while let Some(event) = events.next().await {
    println!("{:?}", event?);
}

// Log streaming (admin)
let mut logs = formation.stream_logs(Some("info")).await?;
while let Some(log) = logs.next().await {
    println!("{:?}", log?);
}
```

## Auth & Headers

- **ServerClient**: HMAC with `key_id`/`secret_key` on `/rpc` endpoints.
- **FormationClient**: `X-MUXI-CLIENT-KEY` or `X-MUXI-ADMIN-KEY` on `/api/{formation}/v1`. Override `base_url` for direct access (e.g., `http://localhost:9012/v1`).
- **Idempotency**: `X-Muxi-Idempotency-Key` auto-generated on every request.
- **SDK headers**: `X-Muxi-SDK`, `X-Muxi-Client` set automatically.

## Timeouts & Retries

- Default timeout: 30s (no timeout for streaming).
- Retries: `max_retries` with exponential backoff on 429/5xx/connection errors; respects `Retry-After`.

## Error Handling

```rust
use muxi::errors::*;

match formation.chat(request, "user-123").await {
    Ok(response) => println!("{:?}", response),
    Err(MuxiError::Authentication(e)) => {
        println!("Auth failed: {}", e);
    }
    Err(MuxiError::RateLimit { retry_after, .. }) => {
        println!("Rate limited. Retry after: {:?}s", retry_after);
    }
    Err(MuxiError::NotFound(e)) => {
        println!("Not found: {}", e);
    }
    Err(e) => {
        println!("Error: {}", e);
    }
}
```

Error types: `Authentication`, `Authorization`, `NotFound`, `Validation`, `RateLimit`, `Server`, `Conflict`, `Connection`.

## Notable Endpoints (FormationClient)

| Category | Methods |
|----------|---------|
| Chat/Audio | `chat`, `chat_stream`, `audio_chat`, `audio_chat_stream` |
| Memory | `get_memory_config`, `get_memories`, `add_memory`, `delete_memory`, `get_user_buffer`, `clear_user_buffer`, `clear_session_buffer`, `clear_all_buffers`, `get_buffer_stats` |
| Scheduler | `get_scheduler_config`, `get_scheduler_jobs`, `get_scheduler_job`, `create_scheduler_job`, `delete_scheduler_job` |
| Sessions | `get_sessions`, `get_session`, `get_session_messages`, `restore_session` |
| Requests | `get_requests`, `get_request_status`, `cancel_request` |
| Agents/MCP | `get_agents`, `get_agent`, `get_mcp_servers`, `get_mcp_server`, `get_mcp_tools` |
| Secrets | `get_secrets`, `get_secret`, `set_secret`, `delete_secret` |
| Credentials | `list_credential_services`, `list_credentials`, `get_credential`, `create_credential`, `delete_credential` |
| Identifiers | `get_user_identifiers_for_user`, `link_user_identifier`, `unlink_user_identifier` |
| Triggers/SOP | `get_triggers`, `get_trigger`, `fire_trigger`, `get_sops`, `get_sop` |
| Audit | `get_audit_log`, `clear_audit_log` |
| Config | `get_status`, `get_config`, `get_formation_info`, `get_async_config`, `get_a2a_config`, `get_logging_config`, `get_logging_destinations`, `get_overlord_config`, `get_overlord_persona`, `get_llm_settings` |
| Streaming | `stream_events`, `stream_logs`, `stream_request` |
| User | `resolve_user` |

## Webhook Verification

```rust
use muxi::webhook::{Webhook, WebhookEvent};

// In your HTTP handler
async fn handle_webhook(payload: &str, signature: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let secret = std::env::var("WEBHOOK_SECRET").ok();

    if !Webhook::verify_signature(payload, signature, secret.as_deref()) {
        return Err("Invalid signature".into());
    }

    let event: WebhookEvent = Webhook::parse(payload)?;

    match event.status.as_str() {
        "completed" => {
            for item in &event.content {
                if item.content_type == "text" {
                    println!("{}", item.text.as_deref().unwrap_or_default());
                }
            }
        }
        "failed" => {
            if let Some(error) = &event.error {
                println!("Error: {}", error.message);
            }
        }
        "awaiting_clarification" => {
            if let Some(clarification) = &event.clarification {
                println!("Question: {}", clarification.question);
            }
        }
        _ => {}
    }

    Ok(())
}
```

## Testing Locally

```bash
cd rust
cargo test
```
