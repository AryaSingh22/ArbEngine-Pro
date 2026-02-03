//! Wallet Management (Simulation)
//!
//! Handles wallet configuration for simulated trading environment.
//! Supports SDK Keypairs for live signing while retaining simulated defaults.

use anyhow::{anyhow, Result};
use std::env;
use tracing::{info, warn};
use solana_sdk::signature::{Keypair, Signer};

/// Wallet wrapper for simulation
pub struct Wallet {
    pub pubkey: String,
    keypair: Option<Keypair>,
}

impl Wallet {
    /// Load wallet public key from environment or generate dummy
    pub fn new() -> Result<Self> {
        let pk_str = env::var("PRIVATE_KEY").ok();

        let (pubkey, keypair) = if let Some(pk) = pk_str {
            if pk.is_empty() {
                ("SimulatedWallet1111111111111111111111111111111".to_string(), None)
            } else {
                match Self::parse_keypair(&pk) {
                    Ok(kp) => (kp.pubkey().to_string(), Some(kp)),
                    Err(err) => {
                        warn!("Failed to parse PRIVATE_KEY: {}. Using simulated wallet.", err);
                        ("SimulatedWallet1111111111111111111111111111111".to_string(), None)
                    }
                }
            }
        } else {
            warn!("PRIVATE_KEY not set. Using simulated wallet.");
            ("SimulatedWallet1111111111111111111111111111111".to_string(), None)
        };

        info!("Wallet loaded: {}", pubkey);
        Ok(Self { pubkey, keypair })
    }

    pub fn pubkey(&self) -> String {
        self.pubkey.clone()
    }

    pub fn signer(&self) -> Option<&Keypair> {
        self.keypair.as_ref()
    }

    fn parse_keypair(value: &str) -> Result<Keypair> {
        if value.trim_start().starts_with('[') {
            let bytes: Vec<u8> = serde_json::from_str(value)?;
            return Keypair::from_bytes(&bytes)
                .map_err(|e| anyhow!("Invalid keypair bytes: {}", e));
        }

        let decoded = bs58::decode(value).into_vec()?;
        Keypair::from_bytes(&decoded).map_err(|e| anyhow!("Invalid base58 keypair: {}", e))
    }
}
