use hmac::{Hmac, Mac};
use sha2::Sha256;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

pub struct Auth;

impl Auth {
    pub fn generate_hmac_signature(secret_key: &str, method: &str, path: &str) -> (String, String) {
        let clean_path = path.split('?').next().unwrap_or(path);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();
        
        let message = format!("{};{};{}", timestamp, method, clean_path);
        
        let mut mac = HmacSha256::new_from_slice(secret_key.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(message.as_bytes());
        let result = mac.finalize();
        let signature = STANDARD.encode(result.into_bytes());
        
        (signature, timestamp)
    }
    
    pub fn build_auth_header(key_id: &str, secret_key: &str, method: &str, path: &str) -> String {
        let (signature, timestamp) = Self::generate_hmac_signature(secret_key, method, path);
        format!(
            "MUXI-HMAC key={}, timestamp={}, signature={}",
            key_id, timestamp, signature
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_generate_hmac_signature() {
        let (sig, ts) = Auth::generate_hmac_signature("secret456", "GET", "/rpc/status");
        assert!(!sig.is_empty());
        assert!(!ts.is_empty());
    }
    
    #[test]
    fn test_build_auth_header() {
        let header = Auth::build_auth_header("key123", "secret456", "GET", "/path");
        assert!(header.contains("MUXI-HMAC key="));
        assert!(header.contains("key123"));
        assert!(header.contains("timestamp="));
        assert!(header.contains("signature="));
    }
    
    #[test]
    fn test_signature_strips_query_params() {
        let (sig1, _) = Auth::generate_hmac_signature("secret", "GET", "/path");
        let (sig2, _) = Auth::generate_hmac_signature("secret", "GET", "/path?foo=bar");
        assert!(!sig1.is_empty());
        assert!(!sig2.is_empty());
    }
}
