//! Jito MEV Protection Module
//!
//! Submits transactions as Jito bundles to protect against MEV/sandwich attacks.
//! This is an optional module — enable via USE_JITO=true in .env.
//!
//! How it works:
//! - Instead of sending transactions directly to the RPC, we send them to
//!   Jito's block engine as a "bundle" with a tip to the validator.
//! - The validator processes the bundle atomically, preventing other transactions
//!   from being inserted between our swap legs.

use anyhow::{anyhow, Result};
use base64::engine::general_purpose::STANDARD as BASE64_ENGINE;
use base64::Engine;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, debug};

/// Jito block engine client for bundle submission
#[derive(Debug, Clone)]
pub struct JitoClient {
    client: Client,
    block_engine_url: String,
    tip_lamports: u64,
}

#[derive(Debug, Serialize)]
struct BundleRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    params: Vec<Vec<String>>, // Array of base64-encoded transactions
}

#[derive(Debug, Deserialize)]
struct BundleResponse {
    result: Option<String>, // Bundle ID
    error: Option<BundleError>,
}

#[derive(Debug, Deserialize)]
struct BundleError {
    message: String,
}

impl JitoClient {
    pub fn new(block_engine_url: &str, tip_lamports: u64) -> Self {
        Self {
            client: Client::new(),
            block_engine_url: block_engine_url.to_string(),
            tip_lamports,
        }
    }

    /// Submit a transaction as a Jito bundle
    ///
    /// The transaction should already be signed. This wraps it in a bundle
    /// and sends it to the Jito block engine.
    pub async fn send_bundle(&self, signed_tx_base64: &str) -> Result<String> {
        info!(
            "📦 Submitting Jito bundle (tip: {} lamports) to {}",
            self.tip_lamports, self.block_engine_url
        );

        let bundle_req = BundleRequest {
            jsonrpc: "2.0".to_string(),
            id: 1,
            method: "sendBundle".to_string(),
            params: vec![vec![signed_tx_base64.to_string()]],
        };

        let url = format!("{}/api/v1/bundles", self.block_engine_url);
        debug!("Jito bundle endpoint: {}", url);

        let response = self.client
            .post(&url)
            .json(&bundle_req)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            return Err(anyhow!("Jito bundle submission failed ({}): {}", status, error_text));
        }

        let bundle_resp: BundleResponse = response.json().await?;

        if let Some(error) = bundle_resp.error {
            warn!("❌ Jito bundle error: {}", error.message);
            return Err(anyhow!("Jito bundle error: {}", error.message));
        }

        match bundle_resp.result {
            Some(bundle_id) => {
                info!("✅ Jito bundle accepted: {}", bundle_id);
                Ok(bundle_id)
            }
            None => Err(anyhow!("Jito bundle returned no result and no error")),
        }
    }

    /// Check if the Jito block engine is reachable
    pub async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/api/v1/bundles", self.block_engine_url);
        match self.client.get(&url).send().await {
            Ok(resp) => Ok(resp.status().is_success() || resp.status().as_u16() == 405),
            Err(e) => {
                warn!("Jito health check failed: {}", e);
                Ok(false)
            }
        }
    }

    /// Get the tip amount in lamports
    pub fn tip_lamports(&self) -> u64 {
        self.tip_lamports
    }
}
