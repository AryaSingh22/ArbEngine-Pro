use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Jito tip floor API: rolling percentiles of recently landed tips, in SOL.
const JITO_TIP_FLOOR_URL: &str = "https://bundles.jito.wtf/api/v1/bundles/tip_floor";

/// How long a fetched tip floor stays fresh before re-querying.
const TIP_CACHE_TTL: Duration = Duration::from_secs(5);

/// Jito block engine client for bundle submission
#[derive(Debug, Clone)]
pub struct JitoClient {
    client: Client,
    block_engine_url: String,
    /// Static tip used as the minimum bid and as fallback when the tip floor
    /// API is unreachable.
    tip_lamports: u64,
    /// Bid from live landed-tip percentiles instead of a fixed amount. A
    /// static bid stops clearing the auction exactly when opportunity density
    /// peaks, so this defaults to on.
    dynamic_tips: bool,
    /// Which landed-tip percentile to bid (25/50/75/95/99).
    tip_percentile: u8,
    /// Upper bound on any tip bid, in lamports.
    max_tip_lamports: u64,
    /// Last fetched tip and when it was fetched.
    cached_tip: Arc<RwLock<Option<(u64, Instant)>>>,
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

#[derive(Debug, Serialize)]
struct BundleStatusRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    params: Vec<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct BundleStatusResponse {
    result: Option<BundleStatusResult>,
    error: Option<BundleError>,
}

#[derive(Debug, Deserialize)]
struct BundleStatusResult {
    value: Vec<Option<BundleStatusData>>,
}

#[derive(Debug, Deserialize)]
pub struct BundleStatusData {
    pub bundle_id: String,
    pub confirmation_status: String,
    pub err: Option<serde_json::Value>,
}

impl JitoClient {
    /// Creates a client. Dynamic tip behaviour is read from the environment:
    /// JITO_DYNAMIC_TIPS (default true), JITO_TIP_PERCENTILE (25/50/75/95/99,
    /// default 75), JITO_MAX_TIP_LAMPORTS (default 1_000_000 = 0.001 SOL).
    /// `tip_lamports` remains the minimum bid and offline fallback.
    pub fn new(block_engine_url: &str, tip_lamports: u64) -> Self {
        let dynamic_tips = std::env::var("JITO_DYNAMIC_TIPS")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(true);
        let tip_percentile: u8 = std::env::var("JITO_TIP_PERCENTILE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(75);
        let tip_percentile = if matches!(tip_percentile, 25 | 50 | 75 | 95 | 99) {
            tip_percentile
        } else {
            warn!(
                "Invalid JITO_TIP_PERCENTILE {} (expected 25/50/75/95/99), using 75",
                tip_percentile
            );
            75
        };
        let max_tip_lamports: u64 = std::env::var("JITO_MAX_TIP_LAMPORTS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1_000_000);

        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap_or_default(),
            block_engine_url: block_engine_url.to_string(),
            tip_lamports,
            dynamic_tips,
            tip_percentile,
            max_tip_lamports,
            cached_tip: Arc::new(RwLock::new(None)),
        }
    }

    /// Submit a transaction as a Jito bundle
    pub async fn send_bundle(&self, signed_tx_base64: &str) -> Result<String> {
        info!("📦 Submitting Jito bundle to {}", self.block_engine_url);

        let bundle_req = BundleRequest {
            jsonrpc: "2.0".to_string(),
            id: 1,
            method: "sendBundle".to_string(),
            params: vec![vec![signed_tx_base64.to_string()]],
        };

        let url = format!("{}/api/v1/bundles", self.block_engine_url);
        debug!("Jito bundle endpoint: {}", url);

        let response = self.client.post(&url).json(&bundle_req).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            return Err(anyhow!(
                "Jito bundle submission failed ({}): {}",
                status,
                error_text
            ));
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

    /// Check the status of a submitted bundle
    pub async fn get_bundle_status(&self, bundle_id: &str) -> Result<Option<BundleStatusData>> {
        let req = BundleStatusRequest {
            jsonrpc: "2.0".to_string(),
            id: 1,
            method: "getBundleStatuses".to_string(),
            params: vec![vec![bundle_id.to_string()]],
        };

        let url = format!("{}/api/v1/bundles", self.block_engine_url);
        let response = self.client.post(&url).json(&req).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Jito bundle status failed: {}", error_text));
        }

        let resp: BundleStatusResponse = response.json().await?;
        if let Some(err) = resp.error {
            return Err(anyhow!("Jito bundle status error: {}", err.message));
        }

        if let Some(res) = resp.result {
            if let Some(Some(data)) = res.value.into_iter().next() {
                return Ok(Some(data));
            }
        }
        
        Ok(None)
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

    /// Get random tip account (Placeholder - normally fetched from Jito API)
    pub async fn get_tip_account(&self) -> Result<String> {
        // List of common Jito tip accounts
        let tip_accounts = ["96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5",
            "HFqU5x63VTqvQss8hp11i4wVV8bD44Puy60pxTKAW4PH",
            "Cw8CFyM9FkoMi7K7Crf6HNQqf4uEMzpKw6QNghXLvLkY",
            "ADaUMid9yfUytqMBgopwjb2DTLSokTSzL1zt6iGPaS49",
            "DfXygSm4jCyNCyb3qzK6966vGgy5tQSZHarris11tc66",
            "ADuUkR4ykG49cvq5RTu3TRLpVIUwDiIHjYyC1E1AtDyV",
            "DttWaMuVvTiduZRnguLF7jNxTgiMBZ1hyAumKUiL2KRL",
            "3AVi9Tg9Uo68tJfuvoKvqKNWKkC5wPdSSdeBnIzKZ6jJ"];

        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        // Safety: tip_accounts is a non-empty compile-time constant
        Ok(tip_accounts
            .choose(&mut rng)
            .expect("tip_accounts is non-empty")
            .to_string())
    }

    /// Get the static/fallback tip amount in lamports
    pub fn tip_lamports(&self) -> u64 {
        self.tip_lamports
    }

    /// Get the tip to bid, in lamports.
    ///
    /// With dynamic tips enabled, bids the configured percentile of recently
    /// landed tips (cached for a few seconds), clamped between the static tip
    /// and `max_tip_lamports`. Falls back to the static tip when the tip
    /// floor API is unreachable.
    pub async fn get_dynamic_tip_lamports(&self) -> u64 {
        if !self.dynamic_tips {
            return self.tip_lamports;
        }

        if let Some((tip, fetched_at)) = *self.cached_tip.read().await {
            if fetched_at.elapsed() < TIP_CACHE_TTL {
                return tip;
            }
        }

        match self.fetch_tip_floor().await {
            Ok(tip) => {
                *self.cached_tip.write().await = Some((tip, Instant::now()));
                debug!(
                    "💸 Dynamic Jito tip: {} lamports ({}th percentile of landed tips)",
                    tip, self.tip_percentile
                );
                tip
            }
            Err(e) => {
                warn!(
                    "Tip floor fetch failed ({}); falling back to static tip {} lamports",
                    e, self.tip_lamports
                );
                self.tip_lamports
            }
        }
    }

    /// Fetch the configured landed-tip percentile from the Jito tip floor API
    /// and convert it from SOL to clamped lamports.
    async fn fetch_tip_floor(&self) -> Result<u64> {
        let response = self.client.get(JITO_TIP_FLOOR_URL).send().await?;
        if !response.status().is_success() {
            return Err(anyhow!("tip floor API returned {}", response.status()));
        }

        let value: serde_json::Value = response.json().await?;
        // The API wraps the snapshot in a one-element array.
        let entry = match value.as_array() {
            Some(arr) => arr
                .first()
                .ok_or_else(|| anyhow!("tip floor API returned empty array"))?,
            None => &value,
        };

        let field = match self.tip_percentile {
            25 => "landed_tips_25th_percentile",
            50 => "landed_tips_50th_percentile",
            95 => "landed_tips_95th_percentile",
            99 => "landed_tips_99th_percentile",
            _ => "landed_tips_75th_percentile",
        };

        let tip_sol = entry
            .get(field)
            .and_then(|v| v.as_f64())
            .ok_or_else(|| anyhow!("tip floor response missing field {}", field))?;

        let lamports = (tip_sol * 1_000_000_000.0) as u64;
        // The static tip acts as the minimum bid; never exceed the cap.
        let max = self.max_tip_lamports.max(self.tip_lamports);
        Ok(lamports.clamp(self.tip_lamports, max))
    }
}
