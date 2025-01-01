use anyhow::Result;
use rand_core::OsRng;
use russh::keys::{Algorithm, PrivateKey};
use std::{ffi::OsString, fs};

pub mod config;
pub mod server;

// Only supports OpenSSH PEM files currently
pub fn get_server_keys(paths: &Vec<String>) -> Result<Vec<PrivateKey>> {
    // Case where default config is used
    // (No server key provided)
    if paths.is_empty() {
        return Ok(vec![
            PrivateKey::random(&mut OsRng, Algorithm::Ed25519).unwrap()
        ]);
    }

    let mut keys: Vec<PrivateKey> = Vec::new();

    for path in paths {
        let path = OsString::from(path);
        let data = fs::read(path)?;
        let key = PrivateKey::from_openssh(data)?;
        keys.push(key);
    }

    Ok(keys)
}
