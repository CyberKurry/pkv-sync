pub mod notifications;
pub mod tools;
pub mod transport_http;
pub mod transport_stdio;

use crate::service::AppState;
use anyhow::Result;
use std::net::SocketAddr;

pub enum McpTransport {
    Stdio {
        vault_id: String,
        token: String,
    },
    Http {
        bind: SocketAddr,
        deployment_key: String,
    },
}

pub async fn run(state: AppState, transport: McpTransport) -> Result<()> {
    match transport {
        McpTransport::Stdio { vault_id, token } => {
            transport_stdio::run(state, vault_id, token).await
        }
        McpTransport::Http {
            bind,
            deployment_key,
        } => transport_http::run(state, bind, deployment_key).await,
    }
}
