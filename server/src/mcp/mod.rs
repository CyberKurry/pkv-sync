pub(crate) mod auth;
pub mod notifications;
pub mod protocol;
pub mod tools;
pub mod transport_http;
pub mod transport_stdio;

use crate::service::AppState;
use anyhow::Result;

pub enum McpTransport {
    Stdio { vault_id: String, token: String },
}

pub async fn run(state: AppState, transport: McpTransport) -> Result<()> {
    match transport {
        McpTransport::Stdio { vault_id, token } => {
            transport_stdio::run(state, vault_id, token).await
        }
    }
}
