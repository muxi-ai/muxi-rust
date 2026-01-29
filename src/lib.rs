pub mod auth;
pub mod errors;
pub mod server_client;
pub mod formation_client;
pub mod webhook;

pub use auth::Auth;
pub use errors::*;
pub use server_client::{ServerClient, ServerConfig};
pub use formation_client::{FormationClient, FormationConfig};
pub use webhook::Webhook;

pub const VERSION: &str = "0.1.0";

#[derive(Debug, Clone)]
pub struct SseEvent {
    pub event: String,
    pub data: String,
}
