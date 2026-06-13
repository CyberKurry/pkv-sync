pub mod auth;
pub(crate) mod background;
pub mod cleanup;
pub mod diff;
pub mod events;
pub mod exclude;
pub mod gc;
pub mod history;
pub mod merge;
pub mod metrics;
pub mod state;
pub mod sync;
pub mod update_check;
pub mod upgrade_signal;
pub mod vault;
pub mod vault_settings;

pub use state::AppState;
