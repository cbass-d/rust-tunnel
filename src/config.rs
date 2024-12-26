use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct RustTunnelConfig {
    pub port: u16,
    // Vector of file paths that point to the server key (OpenSSH PEM files only for now)
    pub server_keys: Vec<String>,
    pub inactivity_timeout: u64,
    pub rejection_time: u64,
    pub routes: Vec<String>,
}

impl std::default::Default for RustTunnelConfig {
    fn default() -> RustTunnelConfig {
        RustTunnelConfig {
            port: 2222,
            server_keys: vec![],
            inactivity_timeout: 3600,
            rejection_time: 3,
            routes: vec![],
        }
    }
}
