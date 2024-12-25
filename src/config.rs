use rand_core::OsRng;
use russh::keys::PrivateKey;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
pub struct RustTunnelConfig {
    pub port: u16,
    pub server_keys: Vec<Vec<u8>>,
    pub inactivity_timeout: Duration,
    pub rejection_time: Duration,
}

impl std::default::Default for RustTunnelConfig {
    fn default() -> RustTunnelConfig {
        RustTunnelConfig {
            port: 2222,
            server_keys: vec![
                PrivateKey::random(&mut OsRng, russh::keys::Algorithm::Ed25519)
                    .unwrap()
                    .to_bytes()
                    .unwrap()
                    .to_vec(),
            ],
            inactivity_timeout: Duration::from_secs(3600),
            rejection_time: Duration::from_secs(3),
        }
    }
}
