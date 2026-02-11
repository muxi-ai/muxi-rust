use crate::{auth::Auth, errors::{MuxiError, Result}, SseEvent, VERSION, version_check};
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use futures::Stream;
use async_stream::stream;

fn parse_sse_events(text: &str) -> Vec<SseEvent> {
    let mut events = Vec::new();
    let mut current_event: Option<String> = None;
    let mut data_parts: Vec<String> = Vec::new();
    
    for line in text.lines() {
        if line.starts_with(':') { continue; }
        if line.is_empty() {
            if !data_parts.is_empty() {
                events.push(SseEvent {
                    event: current_event.take().unwrap_or_else(|| "message".to_string()),
                    data: data_parts.join("\n"),
                });
                data_parts.clear();
            }
            continue;
        }
        if let Some(evt) = line.strip_prefix("event:") { current_event = Some(evt.trim().to_string()); }
        else if let Some(d) = line.strip_prefix("data:") { data_parts.push(d.trim().to_string()); }
    }
    events
}

#[derive(Clone)]
pub struct ServerConfig {
    pub url: String,
    pub key_id: String,
    pub secret_key: String,
    pub timeout: u64,
    pub max_retries: u32,
    pub(crate) app: Option<String>,  // Internal: for Console telemetry
}

impl ServerConfig {
    pub fn new(url: &str, key_id: &str, secret_key: &str) -> Self {
        Self {
            url: url.trim_end_matches('/').to_string(),
            key_id: key_id.to_string(),
            secret_key: secret_key.to_string(),
            timeout: 30,
            max_retries: 0,
            app: None,
        }
    }
}

#[derive(Clone)]
pub struct ServerClient {
    config: ServerConfig,
    client: Client,
}

impl ServerClient {
    pub fn new(config: ServerConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout))
            .build()?;
        Ok(Self { config, client })
    }
    
    pub async fn health(&self) -> Result<Value> { self.get("/health", false).await }
    pub async fn status(&self) -> Result<Value> { self.rpc_get("/rpc/server/status").await }
    pub async fn list_formations(&self) -> Result<Value> { self.rpc_get("/rpc/formations").await }
    pub async fn get_formation(&self, formation_id: &str) -> Result<Value> { self.rpc_get(&format!("/rpc/formations/{}", formation_id)).await }
    pub async fn stop_formation(&self, formation_id: &str) -> Result<Value> { self.rpc_post(&format!("/rpc/formations/{}/stop", formation_id), json!({})).await }
    pub async fn start_formation(&self, formation_id: &str) -> Result<Value> { self.rpc_post(&format!("/rpc/formations/{}/start", formation_id), json!({})).await }
    pub async fn restart_formation(&self, formation_id: &str) -> Result<Value> { self.rpc_post(&format!("/rpc/formations/{}/restart", formation_id), json!({})).await }
    pub async fn rollback_formation(&self, formation_id: &str) -> Result<Value> { self.rpc_post(&format!("/rpc/formations/{}/rollback", formation_id), json!({})).await }
    pub async fn delete_formation(&self, formation_id: &str) -> Result<Value> { self.rpc_delete(&format!("/rpc/formations/{}", formation_id)).await }
    pub async fn cancel_update(&self, formation_id: &str) -> Result<Value> { self.rpc_post(&format!("/rpc/formations/{}/cancel-update", formation_id), json!({})).await }
    pub async fn deploy_formation(&self, formation_id: &str, payload: Value) -> Result<Value> { self.rpc_post(&format!("/rpc/formations/{}/deploy", formation_id), payload).await }
    pub async fn update_formation(&self, formation_id: &str, payload: Value) -> Result<Value> { self.rpc_post(&format!("/rpc/formations/{}/update", formation_id), payload).await }
    
    pub async fn get_formation_logs(&self, formation_id: &str, limit: Option<u32>) -> Result<Value> {
        let path = match limit {
            Some(l) => format!("/rpc/formations/{}/logs?limit={}", formation_id, l),
            None => format!("/rpc/formations/{}/logs", formation_id),
        };
        self.rpc_get(&path).await
    }
    
    pub async fn get_server_logs(&self, limit: Option<u32>) -> Result<Value> {
        let path = match limit {
            Some(l) => format!("/rpc/server/logs?limit={}", l),
            None => "/rpc/server/logs".to_string(),
        };
        self.rpc_get(&path).await
    }
    
    pub fn deploy_formation_stream(&self, formation_id: &str, payload: Value) -> impl Stream<Item = Result<SseEvent>> + '_ {
        let path = format!("/rpc/formations/{}/deploy/stream", formation_id);
        let client = self.client.clone();
        let config = self.config.clone();
        stream! {
            let url = format!("{}{}", config.url, path);
            let resp = client.post(&url)
                .header("X-Muxi-SDK", format!("rust/{}", VERSION))
                .header("Authorization", Auth::build_auth_header(&config.key_id, &config.secret_key, "POST", &path))
                .header("Accept", "text/event-stream")
                .header("Content-Type", "application/json")
                .json(&payload)
                .send()
                .await;
            
            match resp {
                Ok(r) => {
                    let text = r.text().await.unwrap_or_default();
                    for event in parse_sse_events(&text) { yield Ok(event); }
                }
                Err(e) => yield Err(MuxiError::Request(e)),
            }
        }
    }
    
    pub fn stream_formation_logs(&self, formation_id: &str) -> impl Stream<Item = Result<SseEvent>> + '_ {
        let path = format!("/rpc/formations/{}/logs/stream", formation_id);
        let client = self.client.clone();
        let config = self.config.clone();
        stream! {
            let url = format!("{}{}", config.url, path);
            let resp = client.get(&url)
                .header("X-Muxi-SDK", format!("rust/{}", VERSION))
                .header("Authorization", Auth::build_auth_header(&config.key_id, &config.secret_key, "GET", &path))
                .header("Accept", "text/event-stream")
                .send()
                .await;
            
            match resp {
                Ok(r) => {
                    let text = r.text().await.unwrap_or_default();
                    for event in parse_sse_events(&text) { yield Ok(event); }
                }
                Err(e) => yield Err(MuxiError::Request(e)),
            }
        }
    }
    
    async fn get(&self, path: &str, auth: bool) -> Result<Value> {
        let url = format!("{}{}", self.config.url, path);
        let mut req = self.client.get(&url)
            .header("X-Muxi-SDK", format!("rust/{}", VERSION))
            .header("X-Muxi-Client", format!("rust/{}", VERSION))
            .header("X-Muxi-Idempotency-Key", uuid::Uuid::new_v4().to_string())
            .header("Accept", "application/json");
        
        if auth {
            req = req.header("Authorization", Auth::build_auth_header(&self.config.key_id, &self.config.secret_key, "GET", path));
        }
        
        let resp = req.send().await?;
        self.handle_response(resp).await
    }
    
    async fn rpc_get(&self, path: &str) -> Result<Value> { self.get(path, true).await }
    
    async fn rpc_post(&self, path: &str, body: Value) -> Result<Value> {
        let url = format!("{}{}", self.config.url, path);
        let resp = self.client.post(&url)
            .header("X-Muxi-SDK", format!("rust/{}", VERSION))
            .header("X-Muxi-Client", format!("rust/{}", VERSION))
            .header("X-Muxi-Idempotency-Key", uuid::Uuid::new_v4().to_string())
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .header("Authorization", Auth::build_auth_header(&self.config.key_id, &self.config.secret_key, "POST", path))
            .json(&body)
            .send()
            .await?;
        
        self.handle_response(resp).await
    }
    
    async fn rpc_delete(&self, path: &str) -> Result<Value> {
        let url = format!("{}{}", self.config.url, path);
        let resp = self.client.delete(&url)
            .header("X-Muxi-SDK", format!("rust/{}", VERSION))
            .header("X-Muxi-Client", format!("rust/{}", VERSION))
            .header("X-Muxi-Idempotency-Key", uuid::Uuid::new_v4().to_string())
            .header("Accept", "application/json")
            .header("Authorization", Auth::build_auth_header(&self.config.key_id, &self.config.secret_key, "DELETE", path))
            .send()
            .await?;
        
        self.handle_response(resp).await
    }
    
    fn stream_sse_post<'a>(&'a self, path: &'a str, body: Value) -> impl Stream<Item = Result<SseEvent>> + 'a {
        stream! {
            let url = format!("{}{}", self.config.url, path);
            let resp = self.client.post(&url)
                .header("X-Muxi-SDK", format!("rust/{}", VERSION))
                .header("Authorization", Auth::build_auth_header(&self.config.key_id, &self.config.secret_key, "POST", path))
                .header("Accept", "text/event-stream")
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await;
            
            match resp {
                Ok(r) => {
                    let mut current_event: Option<String> = None;
                    let mut data_parts: Vec<String> = Vec::new();
                    let text = r.text().await.unwrap_or_default();
                    for line in text.lines() {
                        if line.starts_with(':') { continue; }
                        if line.is_empty() {
                            if !data_parts.is_empty() {
                                yield Ok(SseEvent {
                                    event: current_event.take().unwrap_or_else(|| "message".to_string()),
                                    data: data_parts.join("\n"),
                                });
                                data_parts.clear();
                            }
                            continue;
                        }
                        if let Some(evt) = line.strip_prefix("event:") { current_event = Some(evt.trim().to_string()); }
                        else if let Some(d) = line.strip_prefix("data:") { data_parts.push(d.trim().to_string()); }
                    }
                }
                Err(e) => yield Err(MuxiError::Request(e)),
            }
        }
    }
    
    fn stream_sse_get<'a>(&'a self, path: &'a str) -> impl Stream<Item = Result<SseEvent>> + 'a {
        stream! {
            let url = format!("{}{}", self.config.url, path);
            let resp = self.client.get(&url)
                .header("X-Muxi-SDK", format!("rust/{}", VERSION))
                .header("Authorization", Auth::build_auth_header(&self.config.key_id, &self.config.secret_key, "GET", path))
                .header("Accept", "text/event-stream")
                .send()
                .await;
            
            match resp {
                Ok(r) => {
                    let mut current_event: Option<String> = None;
                    let mut data_parts: Vec<String> = Vec::new();
                    let text = r.text().await.unwrap_or_default();
                    for line in text.lines() {
                        if line.starts_with(':') { continue; }
                        if line.is_empty() {
                            if !data_parts.is_empty() {
                                yield Ok(SseEvent {
                                    event: current_event.take().unwrap_or_else(|| "message".to_string()),
                                    data: data_parts.join("\n"),
                                });
                                data_parts.clear();
                            }
                            continue;
                        }
                        if let Some(evt) = line.strip_prefix("event:") { current_event = Some(evt.trim().to_string()); }
                        else if let Some(d) = line.strip_prefix("data:") { data_parts.push(d.trim().to_string()); }
                    }
                }
                Err(e) => yield Err(MuxiError::Request(e)),
            }
        }
    }
    
    async fn handle_response(&self, resp: reqwest::Response) -> Result<Value> {
        let status = resp.status().as_u16();
        
        // Check for SDK updates (non-blocking, once per process)
        let headers: std::collections::HashMap<String, String> = resp.headers()
            .iter()
            .filter_map(|(k, v)| v.to_str().ok().map(|s| (k.to_string(), s.to_string())))
            .collect();
        version_check::check_for_updates(&headers);
        
        let retry_after = resp.headers()
            .get("Retry-After")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok());
        
        let body = resp.text().await.unwrap_or_default();
        
        if status >= 400 {
            let (code, message) = if let Ok(json) = serde_json::from_str::<Value>(&body) {
                (
                    json.get("code").or(json.get("error")).and_then(|v| v.as_str()).map(String::from),
                    json.get("message").and_then(|v| v.as_str()).unwrap_or("Unknown error").to_string(),
                )
            } else {
                (None, "Unknown error".to_string())
            };
            return Err(MuxiError::from_response(status, code, message, retry_after));
        }
        
        if body.is_empty() {
            Ok(json!({}))
        } else {
            Ok(serde_json::from_str(&body)?)
        }
    }
}
