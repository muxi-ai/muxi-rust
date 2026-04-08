use crate::{errors::{MuxiError, Result}, SseEvent, VERSION, version_check};
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use futures::{Stream, StreamExt};
use async_stream::stream;

#[derive(Clone)]
pub struct FormationConfig {
    pub server_url: Option<String>,
    pub formation_id: Option<String>,
    pub base_url: Option<String>,
    pub client_key: Option<String>,
    pub admin_key: Option<String>,
    pub timeout: u64,
    pub mode: String,  // "live" (default) or "draft" for local dev
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
            mode: "live".to_string(),
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
            mode: "live".to_string(),
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
            let prefix = if config.mode == "draft" { "draft" } else { "api" };
            format!("{}/{}/{}/v1", server.trim_end_matches('/'), prefix, formation)
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
    pub async fn restore_session(&self, session_id: &str, user_id: &str, messages: Value) -> Result<Value> {
        self.request("POST", &format!("/sessions/{}/restore", session_id), None, Some(json!({"messages": messages})), false, Some(user_id)).await
    }
    
    // Requests
    pub async fn get_requests(&self, user_id: &str) -> Result<Value> { self.request("GET", "/requests", None, None, false, Some(user_id)).await }
    pub async fn get_request_status(&self, request_id: &str, user_id: &str) -> Result<Value> { self.request("GET", &format!("/requests/{}", request_id), None, None, false, Some(user_id)).await }
    pub async fn cancel_request(&self, request_id: &str, user_id: &str) -> Result<Value> { self.request("DELETE", &format!("/requests/{}", request_id), None, None, false, Some(user_id)).await }
    
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
    pub async fn get_user_buffer(&self, user_id: &str) -> Result<Value> {
        self.request("GET", "/memory/buffer", Some(vec![("user_id", user_id.to_string())]), None, false, None).await
    }
    pub async fn clear_user_buffer(&self, user_id: &str) -> Result<Value> {
        self.request("DELETE", "/memory/buffer", Some(vec![("user_id", user_id.to_string())]), None, false, None).await
    }
    pub async fn clear_all_buffers(&self) -> Result<Value> { self.request("DELETE", "/memory/buffer", None, None, true, None).await }
    pub async fn clear_session_buffer(&self, user_id: &str, session_id: &str) -> Result<Value> {
        self.request("DELETE", &format!("/memory/buffer/{}", session_id), Some(vec![("user_id", user_id.to_string())]), None, false, None).await
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
    pub async fn update_scheduler_job(&self, job_id: &str, updates: Value) -> Result<Value> { self.request("PUT", &format!("/scheduler/jobs/{}", job_id), None, Some(updates), true, None).await }
    pub async fn pause_scheduler_job(&self, job_id: &str) -> Result<Value> { self.request("POST", &format!("/scheduler/jobs/{}/pause", job_id), None, None, true, None).await }
    pub async fn resume_scheduler_job(&self, job_id: &str) -> Result<Value> { self.request("POST", &format!("/scheduler/jobs/{}/resume", job_id), None, None, true, None).await }
    
    // Config endpoints
    pub async fn get_async_config(&self) -> Result<Value> { self.request("GET", "/async", None, None, true, None).await }
    pub async fn get_a2a_config(&self) -> Result<Value> { self.request("GET", "/a2a", None, None, true, None).await }
    pub async fn get_logging_config(&self) -> Result<Value> { self.request("GET", "/logging", None, None, true, None).await }
    pub async fn get_logging_destinations(&self) -> Result<Value> { self.request("GET", "/logging/destinations", None, None, true, None).await }
    pub async fn get_overlord_config(&self) -> Result<Value> { self.request("GET", "/overlord", None, None, true, None).await }
    pub async fn get_overlord_soul(&self) -> Result<Value> { self.request("GET", "/overlord/soul", None, None, true, None).await }
    pub async fn get_llm_settings(&self) -> Result<Value> { self.request("GET", "/llm/settings", None, None, true, None).await }
    
    // Triggers / SOPs / Audit
    pub async fn get_triggers(&self) -> Result<Value> { self.request("GET", "/triggers", None, None, false, None).await }
    pub async fn get_trigger(&self, name: &str) -> Result<Value> { self.request("GET", &format!("/triggers/{}", name), None, None, false, None).await }
    pub async fn fire_trigger(&self, name: &str, data: Value, is_async: bool, user_id: Option<&str>) -> Result<Value> {
        self.request("POST", &format!("/triggers/{}", name), Some(vec![("async", is_async.to_string())]), Some(data), false, user_id).await
    }
    pub async fn get_sops(&self) -> Result<Value> { self.request("GET", "/sops", None, None, false, None).await }
    pub async fn get_sop(&self, name: &str) -> Result<Value> { self.request("GET", &format!("/sops/{}", name), None, None, false, None).await }
    pub async fn get_audit_log(&self) -> Result<Value> { self.request("GET", "/audit", None, None, true, None).await }
    pub async fn clear_audit_log(&self) -> Result<Value> { self.request("DELETE", "/audit?confirm=clear-audit-log", None, None, true, None).await }
    
    // Credentials
    pub async fn list_credential_services(&self) -> Result<Value> { self.request("GET", "/credentials/services", None, None, true, None).await }
    pub async fn list_credentials(&self, user_id: &str) -> Result<Value> { self.request("GET", "/credentials", None, None, false, Some(user_id)).await }
    pub async fn get_credential(&self, credential_id: &str, user_id: &str) -> Result<Value> { self.request("GET", &format!("/credentials/{}", credential_id), None, None, false, Some(user_id)).await }
    pub async fn create_credential(&self, user_id: &str, payload: Value) -> Result<Value> { self.request("POST", "/credentials", None, Some(payload), false, Some(user_id)).await }
    pub async fn delete_credential(&self, credential_id: &str, user_id: &str) -> Result<Value> { self.request("DELETE", &format!("/credentials/{}", credential_id), None, None, false, Some(user_id)).await }
    
    // User identifiers
    pub async fn get_user_identifiers(&self, user_id: &str) -> Result<Value> { self.request("GET", &format!("/users/identifiers/{}", user_id), None, None, true, None).await }
    pub async fn link_user_identifier(&self, muxi_user_id: &str, identifiers: Value) -> Result<Value> {
        self.request("POST", "/users/identifiers", None, Some(json!({"muxi_user_id": muxi_user_id, "identifiers": identifiers})), true, None).await
    }
    pub async fn unlink_user_identifier(&self, identifier: &str) -> Result<Value> { self.request("DELETE", &format!("/users/identifiers/{}", identifier), None, None, true, None).await }
    
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
            let mut req = self.stream_client().post(&url);
            req = self.add_headers(req, use_admin, user_id, true);
            req = req.header("Accept", "text/event-stream").json(&body);
            
            match req.send().await {
                Ok(r) => {
                    let r = match self.ensure_stream_response(r).await {
                        Ok(r) => r,
                        Err(err) => {
                            yield Err(err);
                            return;
                        }
                    };
                    let mut parser = SseEventParser::default();
                    let mut body_stream = r.bytes_stream();
                    while let Some(chunk) = body_stream.next().await {
                        match chunk {
                            Ok(bytes) => match parser.push_chunk(String::from_utf8_lossy(&bytes).as_ref()) {
                                Ok(events) => {
                                    for event in events {
                                        yield Ok(event);
                                    }
                                }
                                Err(err) => {
                                    yield Err(err);
                                    return;
                                }
                            },
                            Err(err) => {
                                yield Err(MuxiError::Request(err));
                                return;
                            }
                        }
                    }
                    match parser.finish() {
                        Ok(events) => {
                            for event in events {
                                yield Ok(event);
                            }
                        }
                        Err(err) => yield Err(err),
                    }
                }
                Err(e) => yield Err(MuxiError::Request(e)),
            }
        }
    }
    
    fn stream_sse_get<'a>(&'a self, path: &'a str, params: Option<Vec<(&'a str, String)>>, use_admin: bool, user_id: Option<&'a str>) -> impl Stream<Item = Result<SseEvent>> + 'a {
        stream! {
            let url = self.build_url(path, params);
            let mut req = self.stream_client().get(&url);
            req = self.add_headers(req, use_admin, user_id, false);
            req = req.header("Accept", "text/event-stream");
            
            match req.send().await {
                Ok(r) => {
                    let r = match self.ensure_stream_response(r).await {
                        Ok(r) => r,
                        Err(err) => {
                            yield Err(err);
                            return;
                        }
                    };
                    let mut parser = SseEventParser::default();
                    let mut body_stream = r.bytes_stream();
                    while let Some(chunk) = body_stream.next().await {
                        match chunk {
                            Ok(bytes) => match parser.push_chunk(String::from_utf8_lossy(&bytes).as_ref()) {
                                Ok(events) => {
                                    for event in events {
                                        yield Ok(event);
                                    }
                                }
                                Err(err) => {
                                    yield Err(err);
                                    return;
                                }
                            },
                            Err(err) => {
                                yield Err(MuxiError::Request(err));
                                return;
                            }
                        }
                    }
                    match parser.finish() {
                        Ok(events) => {
                            for event in events {
                                yield Ok(event);
                            }
                        }
                        Err(err) => yield Err(err),
                    }
                }
                Err(e) => yield Err(MuxiError::Request(e)),
            }
        }
    }
    
    fn build_url(&self, path: &str, params: Option<Vec<(&str, String)>>) -> String {
        let full_path = if path.starts_with('/') { path.to_string() } else { format!("/{}", path) };
        let mut url = format!("{}{}", self.base_url, full_path);
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

    fn stream_client(&self) -> Client {
        Client::builder().build().expect("stream client")
    }

    async fn ensure_stream_response(&self, resp: reqwest::Response) -> std::result::Result<reqwest::Response, MuxiError> {
        if !resp.status().is_client_error() && !resp.status().is_server_error() {
            return Ok(resp);
        }

        let status = resp.status().as_u16();
        let retry_after = resp.headers().get("Retry-After").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok());
        let body = resp.text().await.unwrap_or_default();
        let (code, message) = if let Ok(json) = serde_json::from_str::<Value>(&body) {
            (
                json.get("type")
                    .or(json.get("code"))
                    .or(json.get("error"))
                    .and_then(|v| v.as_str())
                    .map(String::from),
                json.get("error")
                    .or(json.get("message"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error")
                    .to_string(),
            )
        } else {
            (None, if body.is_empty() { "Unknown error".to_string() } else { body })
        };
        Err(MuxiError::from_response(status, code, message, retry_after))
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

#[derive(Default)]
struct SseEventParser {
    buffer: String,
    current_event: Option<String>,
    data_parts: Vec<String>,
}

impl SseEventParser {
    fn push_chunk(&mut self, chunk: &str) -> Result<Vec<SseEvent>> {
        self.buffer.push_str(chunk);
        let mut events = Vec::new();

        while let Some(idx) = self.buffer.find('\n') {
            let mut line = self.buffer[..idx].to_string();
            self.buffer.drain(..=idx);
            if line.ends_with('\r') {
                line.pop();
            }
            if let Some(event) = self.process_line(&line)? {
                events.push(event);
            }
        }

        Ok(events)
    }

    fn finish(&mut self) -> Result<Vec<SseEvent>> {
        let mut events = Vec::new();
        if !self.buffer.is_empty() {
            let line = std::mem::take(&mut self.buffer);
            if let Some(event) = self.process_line(line.trim_end_matches('\r'))? {
                events.push(event);
            }
        }
        if let Some(event) = self.flush_event()? {
            events.push(event);
        }
        Ok(events)
    }

    fn process_line(&mut self, line: &str) -> Result<Option<SseEvent>> {
        if line.starts_with(':') {
            return Ok(None);
        }
        if line.is_empty() {
            return self.flush_event();
        }

        let (field, value) = split_sse_field(line);
        match field {
            "event" => self.current_event = Some(value.to_string()),
            "data" => self.data_parts.push(value.to_string()),
            _ => {}
        }
        Ok(None)
    }

    fn flush_event(&mut self) -> Result<Option<SseEvent>> {
        if self.current_event.is_none() && self.data_parts.is_empty() {
            return Ok(None);
        }

        let event = SseEvent {
            event: self.current_event.take().unwrap_or_else(|| "message".to_string()),
            data: self.data_parts.join("\n"),
        };
        self.data_parts.clear();

        if event.event == "error" {
            return Err(parse_route_error(&event.data));
        }

        Ok(Some(event))
    }
}

fn split_sse_field(line: &str) -> (&str, &str) {
    if let Some((field, value)) = line.split_once(':') {
        (field, value.strip_prefix(' ').unwrap_or(value))
    } else {
        (line, "")
    }
}

fn parse_route_error(data: &str) -> MuxiError {
    if let Ok(json) = serde_json::from_str::<Value>(data) {
        let code = json.get("type")
            .or(json.get("code"))
            .or(json.get("error"))
            .and_then(|v| v.as_str())
            .unwrap_or("STREAM_ERROR")
            .to_string();
        let message = json.get("error")
            .or(json.get("message"))
            .and_then(|v| v.as_str())
            .unwrap_or(if data.is_empty() { "stream error" } else { data })
            .to_string();
        return MuxiError::Unknown { code, message, status: 0 };
    }

    MuxiError::Unknown {
        code: "STREAM_ERROR".to_string(),
        message: if data.is_empty() { "stream error".to_string() } else { data.to_string() },
        status: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flushes_event_only_done_frame() {
        let mut parser = SseEventParser::default();
        assert!(parser.process_line(": keepalive").unwrap().is_none());
        assert!(parser.process_line("").unwrap().is_none());
        assert!(parser.process_line("event: done").unwrap().is_none());

        let event = parser.process_line("").unwrap();
        assert_eq!(event.unwrap().event, "done");
    }

    #[test]
    fn preserves_multiline_data() {
        let mut parser = SseEventParser::default();
        parser.process_line("event: planning").unwrap();
        parser.process_line("data: one").unwrap();
        parser.process_line("data: two").unwrap();

        let event = parser.process_line("").unwrap().unwrap();
        assert_eq!(event.event, "planning");
        assert_eq!(event.data, "one\ntwo");
    }

    #[test]
    fn route_error_becomes_muxi_error() {
        let err = parse_route_error(r#"{"error":"boom","type":"RUNTIME_ERROR"}"#);
        match err {
            MuxiError::Unknown { code, message, status } => {
                assert_eq!(code, "RUNTIME_ERROR");
                assert_eq!(message, "boom");
                assert_eq!(status, 0);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
