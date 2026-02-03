//! Execution Module
//!
//! Handles fetching quotes and swap instructions from aggregators (Jupiter).
//! Implements HTTP-based execution path.

use anyhow::{anyhow, Result};
use base64::engine::general_purpose::STANDARD as BASE64_ENGINE;
use base64::Engine;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use std::collections::HashMap;
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::transaction::VersionedTransaction;
use tracing::{info, debug, warn};

use solana_arb_core::ArbitrageOpportunity;
use crate::wallet::Wallet;

const JUPITER_API_URL: &str = "https://quote-api.jup.ag/v6";

// Token Mints (Mainnet)
const SOL_MINT: &str = "So11111111111111111111111111111111111111112";
const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
const RAY_MINT: &str = "4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R";
const ORCA_MINT: &str = "orcaEKTdK7LKz57vaAYr9QeNsVEPfiu6QeMU1kektZE";

#[derive(Debug, Clone)]
pub struct Executor {
    client: Client,
    token_map: HashMap<String, String>,
}

#[derive(Debug, Serialize)]
struct SwapRequest {
    #[serde(rename = "userPublicKey")]
    user_public_key: String,
    #[serde(rename = "quoteResponse")]
    quote_response: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct SwapResponse {
    #[serde(rename = "swapTransaction")]
    swap_transaction: String, // Base64 encoded transaction
}

impl Executor {
    pub fn new() -> Self {
        let mut token_map = HashMap::new();
        token_map.insert("SOL".to_string(), SOL_MINT.to_string());
        token_map.insert("USDC".to_string(), USDC_MINT.to_string());
        token_map.insert("RAY".to_string(), RAY_MINT.to_string());
        token_map.insert("ORCA".to_string(), ORCA_MINT.to_string());

        Self {
            client: Client::new(),
            token_map,
        }
    }

    /// Get quote from Jupiter
    pub async fn get_quote(&self, input_token: &str, output_token: &str, amount: u64) -> Result<serde_json::Value> {
        // Own the strings to avoid temporary borrow issues
        let input_mint = self.token_map.get(input_token).cloned().unwrap_or_else(|| input_token.to_string());
        let output_mint = self.token_map.get(output_token).cloned().unwrap_or_else(|| output_token.to_string());

        let url = format!(
            "{}/quote?inputMint={}&outputMint={}&amount={}&slippageBps=50",
            JUPITER_API_URL, input_mint, output_mint, amount
        );

        debug!("Fetching quote: {}", url);
        let response = self.client.get(&url).send().await?;
        let quote: serde_json::Value = response.json().await?;
        
        Ok(quote)
    }

    /// Execute a trade (simulated or real)
    pub async fn execute(
        &self, 
        wallet: &Wallet, 
        opp: &ArbitrageOpportunity, 
        amount_usd: Decimal,
        submit: bool,
        rpc_url: &str,
    ) -> Result<String> {
        // For simplicity in this phase, we'll just demonstrate fetching the swap instruction
        // We assume we are swapping the base token.
        
        // 1. Get Quote
        // Amount calculation (rough approx for demo)
        // If Buying SOL with USDC, input is USDC.
        
        let (input_token, output_token) = if opp.buy_dex.to_string() == "Jupiter" {
            // Complex logic needed here to map opportunity to specific swap direction
            // For now, let's just log
            (&opp.pair.quote, &opp.pair.base)
        } else {
            (&opp.pair.quote, &opp.pair.base)
        };

        // Assume amount is in atoms (e.g. USDC = 6 decimals)
        let amount_atoms = (amount_usd * Decimal::from(1_000_000)).to_u64().unwrap_or(1000000);

        let quote = match self.get_quote(input_token, output_token, amount_atoms).await {
            Ok(q) => q,
            Err(e) => {
                warn!("Failed to get quote from Jupiter: {}", e);
                return Ok("Failed to get quote".to_string());
            }
        };

        // 2. Get Swap Transaction
        let swap_req = SwapRequest {
            user_public_key: wallet.pubkey(),
            quote_response: quote,
        };

        debug!("Requesting swap instruction...");
        let response = self.client.post(format!("{}/swap", JUPITER_API_URL))
            .json(&swap_req)
            .send()
            .await?;

        if response.status().is_success() {
            let swap_resp: SwapResponse = response.json().await?;
            info!(
                "✅ Received swap transaction (Base64 length: {})",
                swap_resp.swap_transaction.len()
            );

            if submit {
                let signature = self.submit_swap_transaction(wallet, &swap_resp.swap_transaction, rpc_url)?;
                info!("✅ Swap submitted: {}", signature);
                Ok(signature)
            } else {
                info!("📝 [SIMULATION] Transaction would be signed and sent here.");
                Ok(swap_resp.swap_transaction)
            }
        } else {
            let error_text = response.text().await?;
            warn!("Failed to get swap transaction: {}", error_text);
            Ok("Failed".to_string())
        }
    }

    fn submit_swap_transaction(
        &self,
        wallet: &Wallet,
        encoded_tx: &str,
        rpc_url: &str,
    ) -> Result<String> {
        let signer = wallet
            .signer()
            .ok_or_else(|| anyhow!("No keypair available for signing"))?;

        let tx_bytes = BASE64_ENGINE.decode(encoded_tx)?;
        let tx: VersionedTransaction = bincode::deserialize(&tx_bytes)?;
        let signed_tx = VersionedTransaction::try_new(tx.message, &[signer])?;

        let client = RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());
        let signature = client.send_and_confirm_transaction(&signed_tx)?;
        Ok(signature.to_string())
    }
}
