mod config;
mod hook;
mod settings;
mod store;

pub use config::{ClientConfig, ConfigError, FingerprintSecret};
pub use hook::{HookError, HookInput, HookOutcome, HookProcessor};
pub use settings::{InstallHooksResult, SettingsError, install_claude_hooks};
pub use store::{EventStore, StoreError, StoreStatus, TransactionRecord};
