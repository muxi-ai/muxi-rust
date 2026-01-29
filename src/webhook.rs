use hmac::{Hmac, Mac};
use sha2::Sha256;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

pub struct Webhook;

impl Webhook {
    pub fn verify_signature(payload: &str, header: Option<&str>, secret: &str) -> Result<bool, &'static str> {
        Self::verify_signature_with_tolerance(payload, header, secret, 300)
    }
    
    pub fn verify_signature_with_tolerance(payload: &str, header: Option<&str>, secret: &str, tolerance: u64) -> Result<bool, &'static str> {
        if secret.is_empty() { return Err("Webhook secret is required"); }
        
        let header = match header {
            Some(h) if !h.is_empty() => h,
            _ => return Ok(false),
        };
        
        let mut timestamp: Option<&str> = None;
        let mut signature: Option<&str> = None;
        
        for part in header.split(',') {
            let kv: Vec<&str> = part.splitn(2, '=').collect();
            if kv.len() == 2 {
                match kv[0].trim() {
                    "t" => timestamp = Some(kv[1].trim()),
                    "v1" => signature = Some(kv[1].trim()),
                    _ => {}
                }
            }
        }
        
        let (ts, sig) = match (timestamp, signature) {
            (Some(t), Some(s)) => (t, s),
            _ => return Ok(false),
        };
        
        let ts_num: u64 = ts.parse().map_err(|_| "Invalid timestamp")?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        if now.abs_diff(ts_num) > tolerance { return Ok(false); }
        
        let message = format!("{}.{}", ts, payload);
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(message.as_bytes());
        let result = mac.finalize();
        let expected: String = result.into_bytes().iter().map(|b| format!("{:02x}", b)).collect();
        
        Ok(expected == sig)
    }
    
    pub fn parse(payload: &str) -> Result<WebhookEvent, serde_json::Error> {
        serde_json::from_str(payload)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEvent {
    #[serde(rename = "requestId")]
    pub request_id: Option<String>,
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
    #[serde(rename = "userId")]
    pub user_id: Option<String>,
    pub status: Option<String>,
    pub content: Option<Vec<ContentItem>>,
    pub error: Option<ErrorInfo>,
    pub clarification: Option<ClarificationInfo>,
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentItem {
    #[serde(rename = "type")]
    pub content_type: Option<String>,
    pub text: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorInfo {
    pub code: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClarificationInfo {
    pub question: Option<String>,
    pub options: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_verify_signature_missing_secret() {
        assert!(Webhook::verify_signature("payload", Some("t=123,v1=abc"), "").is_err());
    }
    
    #[test]
    fn test_verify_signature_null_header() {
        assert_eq!(Webhook::verify_signature("payload", None, "secret").unwrap(), false);
    }
    
    #[test]
    fn test_verify_signature_empty_header() {
        assert_eq!(Webhook::verify_signature("payload", Some(""), "secret").unwrap(), false);
    }
    
    #[test]
    fn test_parse_completed_payload() {
        let payload = r#"{"status":"completed","content":[{"type":"text","text":"Hello"}]}"#;
        let event = Webhook::parse(payload).unwrap();
        assert_eq!(event.status, Some("completed".to_string()));
        assert!(event.content.is_some());
    }
    
    #[test]
    fn test_parse_failed_payload() {
        let payload = r#"{"status":"failed","error":{"code":"ERROR","message":"Something went wrong"}}"#;
        let event = Webhook::parse(payload).unwrap();
        assert_eq!(event.status, Some("failed".to_string()));
        assert!(event.error.is_some());
    }
}
