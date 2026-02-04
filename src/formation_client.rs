use crate::{errors::{MuxiError, Result}, SseEvent, VERSION, version_check};
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use futures::Stream;
use async_stream::stream;

#[derive(Clone)]
pub struct FormationConfig {
    pub server_url: Option<String>,
    pub formation_id: Option<String>,
    pub base_url: Option<String>,
    pub client_key: Option<String>,
    pub admin_key: Option<String>,
    pub timeout: u64,
    pub(crate) app: Option<String>,  // Internal: for Console telemetry
}

impl FormationConfig {
    pub fn new(server_url: &str, formation_id: &str, client_key: &str, admin_key: &str) -> Self {
        Self {
            server_url: Some(server_url.to_string()),
            formation_id: Some(formation_id.to_string()),
            base_url: None,
            client_key: Some(client_key.to_string()),
            admin_key: Some(admin_key.to_string()),
            timeout: 30,
            app: None,
        }
    }
    
    pub fn with_base_url(base_url: &str, client_key: &str, admin_key: &str) -> Self {
        Self {
            server_url: None,
            formation_id: None,
            base_url: Some(base_url.to_string()),
            client_key: Some(client_key.to_string()),
            admin_key: Some(admin_key.to_string()),
            timeout: 30,
            app: None,
        }
    }
}

#[derive(Clone)]
pub struct FormationClient {
    base_url: String,
    client_key: Option<String>,
    admin_key: Option<String>,
    app: Option<String>,
    client: Client,
}

impl FormationClient {
    pub fn new(config: FormationConfig) -> Result<Self> {
        let base_url = if let Some(base) = config.base_url {
            base.trim_end_matches('/').to_string()
        } else if let (Some(server), Some(formation)) = (&config.server_url, &config.formation_id) {
            format!("{}/api/{}/v1", server.trim_end_matches('/'), formation)
        } else {
            return Err(MuxiError::Connection("Must provide base_url or server_url+formation_id".to_string()));
        };
        
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout))
            .build()?;
        
        Ok(Self { base_url, client_key: config.client_key, admin_key: config.admin_key, app: config.app, client })
    }
    
    // Health / Status
    pub async fn health(&self) -> Result<Value> { self.request("GET", "/health", None, None, false, None).await }
    pub async fn get_status(&self) -> Result<Value> { self.request("GET", "/status", None, None, true, None).await }
    pub async fn get_config(&self) -> Result<Value> { self.request("GET", "/config", None, None, true, None).await }
    pub async fn get_formation_info(&self) -> Result<Value> { self.request("GET", "/formation", None, None, true, None).await }
    
    // Agents / MCP
    pub async fn get_agents(&self) -> Result<Value> { self.request("GET", "/agents", None, None, true, None).await }
    pub async fn get_agent(&self, agent_id: &str) -> Result<Value> { self.request("GET", &format!("/agents/{}", agent_id), None, None, true, None).await }
    pub async fn get_mcp_servers(&self) -> Result<Value> { self.request("GET", "/mcp/servers", None, None, true, None).await }
    pub async fn get_mcp_server(&self, server_id: &str) -> Result<Value> { self.request("GET", &format!("/mcp/servers/{}", server_id), None, None, true, None).await }
    pub async fn get_mcp_tools(&self) -> Result<Value> { self.request("GET", "/mcp/tools", None, None, true, None).await }
    
    // Secrets
    pub async fn get_secrets(&self) -> Result<Value> { self.request("GET", "/secrets", None, None, true, None).await }
    pub async fn get_secret(&self, key: &str) -> Result<Value> { self.request("GET", &format!("/secrets/{}", key), None, None, true, None).await }
    pub async fn set_secret(&self, key: &str, value: &str) -> Result<Value> { self.request("PUT", &format!("/secrets/{}", key), None, Some(json!({"value": value})), true, None).await }
    pub async fn delete_secret(&self, key: &str) -> Result<Value> { self.request("DELETE", &format!("/secrets/{}", key), None, None, true, None).await }
    
    // Chat
    pub async fn chat(&self, payload: Value, user_id: Option<&str>) -> Result<Value> { self.request("POST", "/chat", None, Some(payload), false, user_id).await }
    pub fn chat_stream<'a>(&'a self, mut payload: Value, user_id: Option<&'a str>) -> impl Stream<Item = Result<SseEvent>> + 'a {
        payload.as_object_mut().map(|o| o.insert("stream".to_string(), json!(true)));
        self.stream_sse_post("/chat", payload, false, user_id)
    }
    pub async fn audio_chat(&self, payload: Value, user_id: Option<&str>) -> Result<Value> { self.request("POST", "/audiochat", None, Some(payload), false, user_id).await }
    pub fn audio_chat_stream<'a>(&'a self, mut payload: Value, user_id: Option<&'a str>) -> impl Stream<Item = Result<SseEvent>> + 'a {
        payload.as_object_mut().map(|o| o.insert("stream".to_string(), json!(true)));
        self.stream_sse_post("/audiochat", payload, false, user_id)
    }
    
    // Sessions
    pub async fn get_sessions(&self, user_id: &str, limit: Option<u32>) -> Result<Value> {
        let mut params = vec![("user_id", user_id.to_string())];
        if let Some(l) = limit { params.push(("limit", l.to_string())); }
        self.request("GET", "/sessions", Some(params), None, false, Some(user_id)).await
    }
    pub async fn get_session(&self, session_id: &str, user_id: &str) -> Result<Value> { self.request("GET", &format!("/sessions/{}", session_id), None, None, false, Some(user_id)).await }
    pub async fn get_session_messages(&self, session_id: &str, user_id: &str) -> Result<Value> { self.request("GET", &format!("/sessions/{}/messages", session_id), None, None, false, Some(user_id)).await }
    
    // Memory
    pub async fn get_memory_config(&self) -> Result<Value> { self.request("GET", "/memory", None, None, true, None).await }
    pub async fn get_memories(&self, user_id: &str, limit: Option<u32>) -> Result<Value> {
        let mut params = vec![("user_id", user_id.to_string())];
        if let Some(l) = limit { params.push(("limit", l.to_string())); }
        self.request("GET", "/memories", Some(params), None, false, Some(user_id)).await
    }
    pub async fn add_memory(&self, user_id: &str, memory_type: &str, detail: &str) -> Result<Value> {
        self.request("POST", "/memories", None, Some(json!({"user_id": user_id, "type": memory_type, "detail": detail})), false, Some(user_id)).await
    }
    pub async fn delete_memory(&self, user_id: &str, memory_id: &str) -> Result<Value> {
        self.request("DELETE", &format!("/memories/{}", memory_id), Some(vec![("user_id", user_id.to_string())]), None, false, Some(user_id)).await
    }
    pub async fn get_buffer_stats(&self) -> Result<Value> { self.request("GET", "/memory/stats", None, None, true, None).await }
    
    // Scheduler
    pub async fn get_scheduler_config(&self) -> Result<Value> { self.request("GET", "/scheduler", None, None, true, None).await }
    pub async fn get_scheduler_jobs(&self, user_id: &str) -> Result<Value> {
        self.request("GET", "/scheduler/jobs", Some(vec![("user_id", user_id.to_string())]), None, true, None).await
    }
    pub async fn get_scheduler_job(&self, job_id: &str) -> Result<Value> { self.request("GET", &format!("/scheduler/jobs/{}", job_id), None, None, true, None).await }
    pub async fn create_scheduler_job(&self, job_type: &str, schedule: &str, message: &str, user_id: &str) -> Result<Value> {
        self.request("POST", "/scheduler/jobs", None, Some(json!({"type": job_type, "schedule": schedule, "message": message, "user_id": user_id})), true, None).await
    }
    pub async fn delete_scheduler_job(&self, job_id: &str) -> Result<Value> { self.request("DELETE", &format!("/scheduler/jobs/{}", job_id), None, None, true, None).await }
    
    // Config endpoints
    pub async fn get_async_config(&self) -> Result<Value> { self.request("GET", "/async", None, None, true, None).await }
    pub async fn get_a2a_config(&self) -> Result<Value> { self.request("GET", "/a2a", None, None, true, None).await }
    pub async fn get_logging_config(&self) -> Result<Value> { self.request("GET", "/logging", None, None, true, None).await }
    pub async fn get_overlord_config(&self) -> Result<Value> { self.request("GET", "/overlord", None, None, true, None).await }
    pub async fn get_llm_settings(&self) -> Result<Value> { self.request("GET", "/llm/settings", None, None, true, None).await }
    
    // Triggers / Audit
    pub async fn get_triggers(&self) -> Result<Value> { self.request("GET", "/triggers", None, None, false, None).await }
    pub async fn get_trigger(&self, name: &str) -> Result<Value> { self.request("GET", &format!("/triggers/{}", name), None, None, false, None).await }
    pub async fn fire_trigger(&self, name: &str, data: Value, is_async: bool, user_id: Option<&str>) -> Result<Value> {
        self.request("POST", &format!("/triggers/{}", name), Some(vec![("async", is_async.to_string())]), Some(data), false, user_id).await
    }
    pub async fn get_audit_log(&self) -> Result<Value> { self.request("GET", "/audit", None, None, true, None).await }
    
    // Streaming
    pub fn stream_events<'a>(&'a self, user_id: &'a str) -> impl Stream<Item = Result<SseEvent>> + 'a {
        self.stream_sse_get("/events", Some(vec![("user_id", user_id.to_string())]), false, Some(user_id))
    }
    pub fn stream_logs<'a>(&'a self, filters: Option<Vec<(&'a str, String)>>) -> impl Stream<Item = Result<SseEvent>> + 'a {
        self.stream_sse_get("/logs", filters, true, None)
    }
    
    // Resolve user
    pub async fn resolve_user(&self, identifier: &str, create_user: bool) -> Result<Value> {
        self.request("POST", "/users/resolve", None, Some(json!({"identifier": identifier, "create_user": create_user})), false, None).await
    }
    
    async fn request(&self, method: &str, path: &str, params: Option<Vec<(&str, String)>>, body: Option<Value>, use_admin: bool, user_id: Option<&str>) -> Result<Value> {
        let url = self.build_url(path, params);
        let mut req = match method {
            "GET" => self.client.get(&url),
            "POST" => self.client.post(&url),
            "PUT" => self.client.put(&url),
            "DELETE" => self.client.delete(&url),
            _ => return Err(MuxiError::Connection(format!("Unknown method: {}", method))),
        };
        
        req = self.add_headers(req, use_admin, user_id, body.is_some());
        if let Some(b) = body { req = req.json(&b); }
        
        let resp = req.send().await?;
        self.handle_response(resp).await
    }
    
    fn stream_sse_post<'a>(&'a self, path: &'a str, body: Value, use_admin: bool, user_id: Option<&'a str>) -> impl Stream<Item = Result<SseEvent>> + 'a {
        stream! {
            let url = self.build_url(path, None);
            let mut req = self.client.post(&url);
            req = self.add_headers(req, use_admin, user_id, true);
            req = req.header("Accept", "text/event-stream").json(&body);
            
            match req.send().await {
                Ok(r) => {
                    let mut current_event: Option<String> = None;
                    let mut data_parts: Vec<String> = Vec::new();
                    let text = r.text().await.unwrap_or_default();
                    for line in text.lines() {
                        if line.starts_with(':') { continue; }
                        if line.is_empty() {
                            if !data_parts.is_empty() {
                                yield Ok(SseEvent { event: current_event.take().unwrap_or_else(|| "message".to_string()), data: data_parts.join("\n") });
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
    
    fn stream_sse_get<'a>(&'a self, path: &'a str, params: Option<Vec<(&'a str, String)>>, use_admin: bool, user_id: Option<&'a str>) -> impl Stream<Item = Result<SseEvent>> + 'a {
        stream! {
            let url = self.build_url(path, params);
            let mut req = self.client.get(&url);
            req = self.add_headers(req, use_admin, user_id, false);
            req = req.header("Accept", "text/event-stream");
            
            match req.send().await {
                Ok(r) => {
                    let mut current_event: Option<String> = None;
                    let mut data_parts: Vec<String> = Vec::new();
                    let text = r.text().await.unwrap_or_default();
                    for line in text.lines() {
                        if line.starts_with(':') { continue; }
                        if line.is_empty() {
                            if !data_parts.is_empty() {
                                yield Ok(SseEvent { event: current_event.take().unwrap_or_else(|| "message".to_string()), data: data_parts.join("\n") });
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
    
    fn build_url(&self, path: &str, params: Option<Vec<(&str, String)>>) -> String {
        let mut url = format!("{}{}", self.base_url, if path.starts_with('/') { path } else { &format!("/{}", path) });
        if let Some(p) = params {
            if !p.is_empty() {
                url.push('?');
                url.push_str(&p.iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<_>>().join("&"));
            }
        }
        url
    }
    
    fn add_headers(&self, req: reqwest::RequestBuilder, use_admin: bool, user_id: Option<&str>, has_body: bool) -> reqwest::RequestBuilder {
        let mut req = req
            .header("X-Muxi-SDK", format!("rust/{}", VERSION))
            .header("X-Muxi-Client", format!("rust/{}", VERSION))
            .header("X-Muxi-Idempotency-Key", uuid::Uuid::new_v4().to_string())
            .header("Accept", "application/json");
        
        if let Some(app) = &self.app { req = req.header("X-Muxi-App", app); }
        if use_admin {
            if let Some(key) = &self.admin_key { req = req.header("X-MUXI-ADMIN-KEY", key); }
        } else {
            if let Some(key) = &self.client_key { req = req.header("X-MUXI-CLIENT-KEY", key); }
        }
        if let Some(uid) = user_id { req = req.header("X-Muxi-User-ID", uid); }
        if has_body { req = req.header("Content-Type", "application/json"); }
        req
    }
    
    async fn handle_response(&self, resp: reqwest::Response) -> Result<Value> {
        let status = resp.status().as_u16();
        
        // Check for SDK updates (non-blocking, once per process)
        let headers: std::collections::HashMap<String, String> = resp.headers()
            .iter()
            .filter_map(|(k, v)| v.to_str().ok().map(|s| (k.to_string(), s.to_string())))
            .collect();
        version_check::check_for_updates(&headers);
        
        let retry_after = resp.headers().get("Retry-After").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok());
        let body = resp.text().await.unwrap_or_default();
        
        if status >= 400 {
            let (code, message) = if let Ok(json) = serde_json::from_str::<Value>(&body) {
                (json.get("code").or(json.get("error")).and_then(|v| v.as_str()).map(String::from), json.get("message").and_then(|v| v.as_str()).unwrap_or("Unknown error").to_string())
            } else { (None, "Unknown error".to_string()) };
            return Err(MuxiError::from_response(status, code, message, retry_after));
        }
        
        if body.is_empty() { Ok(json!({})) } else { Ok(self.unwrap_envelope(serde_json::from_str(&body)?)) }
    }
    
    fn unwrap_envelope(&self, value: Value) -> Value {
        if let Some(obj) = value.as_object() {
            if let Some(data) = obj.get("data") {
                if let Some(mut result) = data.as_object().cloned() {
                    if let Some(req) = obj.get("request").and_then(|r| r.as_object()) {
                        if let Some(id) = req.get("id") { if !result.contains_key("request_id") { result.insert("request_id".to_string(), id.clone()); } }
                    }
                    return Value::Object(result);
                }
                return data.clone();
            }
        }
        value
    }
}
