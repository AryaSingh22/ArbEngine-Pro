//! Execution Module
//!
//! Handles fetching quotes and swap instructions from aggregators (Jupiter).
//! Implements HTTP-based execution path with priority fees, retry logic,
//! and balance checking for production-ready trading.

use anyhow::{anyhow, Result};
use base64::engine::general_purpose::STANDARD as BASE64_ENGINE;
use base64::Engine;
use reqwest::Client;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use solana_rpc_client::rpc_client::RpcClient;
use solana_rpc_client_api::config::RpcSendTransactionConfig;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::compute_budget::ComputeBudgetInstruction;
use solana_sdk::message::{Message, VersionedMessage};
use solana_sdk::signature::Signer;
use solana_sdk::transaction::VersionedTransaction;
use std::collections::HashMap;
use tracing::{debug, error, info, warn};

use crate::wallet::Wallet;
use solana_arb_core::jito::JitoClient;
use solana_arb_core::types::TradeResult;
use solana_arb_core::ArbitrageOpportunity;

use crate::flash_loan_tx_builder::FlashLoanTxBuilder;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use std::str::FromStr;

const JUPITER_API_URL: &str = "https://quote-api.jup.ag/v6";

// Token Mints (Mainnet)
pub const SOL_MINT: &str = "So11111111111111111111111111111111111111112";
pub const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
pub const RAY_MINT: &str = "4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R";
pub const ORCA_MINT: &str = "orcaEKTdK7LKz57vaAYr9QeNsVEPfiu6QeMU1kektZE";

/// Execution configuration
#[derive(Debug, Clone)]
pub struct ExecutionConfig {
    pub priority_fee_micro_lamports: u64,
    pub compute_unit_limit: u32,
    pub slippage_bps: u64,
    pub max_retries: u32,
    pub rpc_commitment: String,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            priority_fee_micro_lamports: 50_000,
            compute_unit_limit: 200_000,
            slippage_bps: 50,
            max_retries: 3,
            rpc_commitment: "confirmed".to_string(),
        }
    }
}

use solana_arb_core::alt::AltManager;
use std::sync::Arc;

#[derive(Debug)]
pub struct Executor {
    client: Client,
    token_map: HashMap<String, String>,
    config: ExecutionConfig,
    flash_loan_builder: FlashLoanTxBuilder,
    flash_loans_enabled: bool,
    alt_manager: Option<Arc<AltManager>>,
}

#[derive(Debug, Serialize)]
struct SwapRequest {
    #[serde(rename = "userPublicKey")]
    user_public_key: String,
    #[serde(rename = "quoteResponse")]
    quote_response: serde_json::Value,
    #[serde(rename = "computeUnitPriceMicroLamports")]
    compute_unit_price_micro_lamports: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct SwapResponse {
    #[serde(rename = "swapTransaction")]
    swap_transaction: String,
}

impl Executor {
    pub fn new() -> Self {
        Self::with_config(ExecutionConfig::default())
    }

    pub fn with_config(config: ExecutionConfig) -> Self {
        let is_devnet = config.rpc_commitment == "devnet"
            || std::env::var("SOLANA_RPC_URL")
                .unwrap_or_default()
                .contains("devnet");

        let mut token_map = HashMap::new();
        if is_devnet {
            // Devnet Mints
            // Solend Devnet USDC: zVzi5VAf4qMEwzv7NXECVx5v2pQ7xnqVVjCXZwS9XzA
            token_map.insert("SOL".to_string(), SOL_MINT.to_string());
            token_map.insert(
                "USDC".to_string(),
                "zVzi5VAf4qMEwzv7NXECVx5v2pQ7xnqVVjCXZwS9XzA".to_string(),
            );
            // Other mints (RAY, ORCA) might not work on Devnet or use different addresses.
            // Leaving them pointing to Mainnet but users should be aware.
            token_map.insert("RAY".to_string(), RAY_MINT.to_string());
            token_map.insert("ORCA".to_string(), ORCA_MINT.to_string());
        } else {
            token_map.insert("SOL".to_string(), SOL_MINT.to_string());
            token_map.insert("USDC".to_string(), USDC_MINT.to_string());
            token_map.insert("RAY".to_string(), RAY_MINT.to_string());
            token_map.insert("ORCA".to_string(), ORCA_MINT.to_string());
        }

        let wallet = crate::wallet::Wallet::new().expect("Failed to load wallet for executor");
        let keypair = if let Some(kp) = wallet.signer() {
            Keypair::from_bytes(&kp.to_bytes()).expect("Failed to clone keypair")
        } else {
            Keypair::new()
        };

        Self {
            client: Client::new(),
            token_map,
            config: config.clone(),
            flash_loan_builder: FlashLoanTxBuilder::new(keypair, is_devnet),
            flash_loans_enabled: std::env::var("ENABLE_FLASH_LOANS").unwrap_or("false".to_string())
                == "true",
            alt_manager: None,
        }
    }

    pub fn set_alt_manager(&mut self, manager: Arc<AltManager>) {
        self.alt_manager = Some(manager);
    }

    pub async fn get_quote(
        &self,
        input_mint: &str,
        output_mint: &str,
        amount: u64,
    ) -> Result<serde_json::Value> {
        let url = format!(
            "{}/quote?inputMint={}&outputMint={}&amount={}&slippageBps={}",
            JUPITER_API_URL, input_mint, output_mint, amount, self.config.slippage_bps
        );

        debug!("Fetching quote from {}", url);
        let response = self.client.get(&url).send().await?;
        if !response.status().is_success() {
            let err_text = response.text().await?;
            return Err(anyhow!("Jupiter quote failed: {}", err_text));
        }
        let quote: serde_json::Value = response.json().await?;
        Ok(quote)
    }

    pub fn check_balance(&self, wallet: &Wallet, rpc_url: &str) -> Result<u64> {
        let client = RpcClient::new(rpc_url.to_string());
        let pubkey = Pubkey::from_str(&wallet.pubkey())
            .map_err(|e| anyhow!("Invalid wallet pubkey: {}", e))?;
        Ok(client.get_balance(&pubkey)?)
    }

    pub async fn execute(
        &self,
        wallet: &Wallet,
        opp: &ArbitrageOpportunity,
        amount_usd: Decimal,
        submit: bool,
        rpc_url: &str,
        jito_client: Option<&JitoClient>,
    ) -> Result<TradeResult> {
        let flash_loan_threshold = Decimal::from(1000);
        let use_flash_loan = self.flash_loans_enabled && amount_usd > flash_loan_threshold;

        if use_flash_loan {
            return self
                .execute_with_flash_loan(wallet, opp, amount_usd, submit, rpc_url, jito_client)
                .await;
        }

        self.execute_standard(wallet, opp, amount_usd, submit, rpc_url, jito_client)
            .await
    }

    pub async fn execute_standard(
        &self,
        wallet: &Wallet,
        opp: &ArbitrageOpportunity,
        amount_usd: Decimal,
        submit: bool,
        rpc_url: &str,
        jito_client: Option<&JitoClient>,
    ) -> Result<TradeResult> {
        let (input_token, output_token) = if opp.buy_dex.to_string() == "Jupiter" {
            (&opp.pair.quote, &opp.pair.base)
        } else {
            (&opp.pair.quote, &opp.pair.base)
        };

        let amount_atoms = (amount_usd * Decimal::from(1_000_000))
            .to_u64()
            .unwrap_or(1_000_000);

        let quote = match self
            .get_quote(input_token, output_token, amount_atoms)
            .await
        {
            Ok(q) => {
                if let Some(out_amount) = q.get("outAmount") {
                    info!(
                        "📊 Quote: {} {} → {} {} (slippage: {}bps)",
                        amount_atoms,
                        input_token,
                        out_amount,
                        output_token,
                        self.config.slippage_bps
                    );
                }
                q
            }
            Err(e) => {
                warn!("Failed to get quote from Jupiter: {}", e);
                return Ok(TradeResult {
                    opportunity_id: opp.id,
                    signature: None,
                    success: false,
                    actual_profit: Decimal::ZERO,
                    executed_at: chrono::Utc::now(),
                    error: Some(format!("Failed to get quote: {}", e)),
                });
            }
        };

        let swap_req = SwapRequest {
            user_public_key: wallet.pubkey(),
            quote_response: quote,
            compute_unit_price_micro_lamports: if submit {
                Some(self.config.priority_fee_micro_lamports)
            } else {
                None
            },
        };

        debug!("Requesting swap instruction...");
        let response = self
            .client
            .post(format!("{}/swap", JUPITER_API_URL))
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
                if let Ok(balance) = self.check_balance(wallet, rpc_url) {
                    let min_balance = 10_000_000;
                    if balance < min_balance {
                        return Ok(TradeResult {
                            opportunity_id: opp.id,
                            signature: None,
                            success: false,
                            actual_profit: Decimal::ZERO,
                            executed_at: chrono::Utc::now(),
                            error: Some("Insufficient SOL balance".to_string()),
                        });
                    }
                }

                match self.submit_with_retry(
                    wallet,
                    &swap_resp.swap_transaction,
                    rpc_url,
                    jito_client,
                ) {
                    Ok(signature) => {
                        info!("✅ Swap submitted: {}", signature);
                        Ok(TradeResult {
                            opportunity_id: opp.id,
                            signature: Some(signature),
                            success: true,
                            actual_profit: opp.estimated_profit_usd.unwrap_or_default(),
                            executed_at: chrono::Utc::now(),
                            error: None,
                        })
                    }
                    Err(e) => Ok(TradeResult {
                        opportunity_id: opp.id,
                        signature: None,
                        success: false,
                        actual_profit: Decimal::ZERO,
                        executed_at: chrono::Utc::now(),
                        error: Some(format!("Submission failed: {}", e)),
                    }),
                }
            } else {
                info!("📝 [SIMULATION] Transaction would be signed and sent here.");
                Ok(TradeResult {
                    opportunity_id: opp.id,
                    signature: Some("simulated_signature".to_string()),
                    success: true,
                    actual_profit: opp.estimated_profit_usd.unwrap_or_default(),
                    executed_at: chrono::Utc::now(),
                    error: None,
                })
            }
        } else {
            let error_text = response.text().await?;
            warn!("Failed to get swap transaction: {}", error_text);
            Ok(TradeResult {
                opportunity_id: opp.id,
                signature: None,
                success: false,
                actual_profit: Decimal::ZERO,
                executed_at: chrono::Utc::now(),
                error: Some(format!("Failed to get swap transaction: {}", error_text)),
            })
        }
    }

    fn submit_with_retry(
        &self,
        wallet: &Wallet,
        encoded_tx: &str,
        rpc_url: &str,
        jito_client: Option<&JitoClient>,
    ) -> Result<String> {
        let mut last_error = None;

        for attempt in 0..self.config.max_retries {
            match self.submit_swap_transaction(wallet, encoded_tx, rpc_url, jito_client) {
                Ok(sig) => return Ok(sig),
                Err(e) => {
                    let delay_ms = 500 * 2u64.pow(attempt);
                    warn!(
                        "⚠️ Transaction attempt {}/{} failed: {}. Retrying in {}ms...",
                        attempt + 1,
                        self.config.max_retries,
                        e,
                        delay_ms
                    );
                    std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("All retry attempts exhausted")))
    }

    fn submit_swap_transaction(
        &self,
        wallet: &Wallet,
        encoded_tx: &str,
        rpc_url: &str,
        jito_client: Option<&JitoClient>,
    ) -> Result<String> {
        let signer = wallet
            .signer()
            .ok_or_else(|| anyhow!("No keypair available for signing"))?;

        let tx_bytes = BASE64_ENGINE.decode(encoded_tx)?;
        let tx: VersionedTransaction = bincode::deserialize(&tx_bytes)?;
        let signed_tx = VersionedTransaction::try_new(tx.message, &[signer])?;

        if let Some(jito) = jito_client {
            let signed_tx_bytes = bincode::serialize(&signed_tx)?;
            let signed_tx_base64 = BASE64_ENGINE.encode(signed_tx_bytes);

            let bundle_id = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(jito.send_bundle(&signed_tx_base64))
            })?;

            info!("🚀 Sent via Jito! Bundle ID: {}", bundle_id);
            return Ok(bundle_id);
        }

        let commitment = self.parse_commitment();
        let client = RpcClient::new_with_commitment(rpc_url.to_string(), commitment);

        let config = RpcSendTransactionConfig {
            skip_preflight: true,
            ..Default::default()
        };

        let signature = client.send_transaction_with_config(&signed_tx, config)?;

        info!(
            "📡 Transaction sent: {}. Waiting for confirmation...",
            signature
        );
        match client.confirm_transaction_with_spinner(
            &signature,
            &client.get_latest_blockhash()?,
            commitment,
        ) {
            Ok(_) => {
                info!("✅ Transaction confirmed: {}", signature);
            }
            Err(e) => {
                error!("⚠️ Transaction sent but confirmation uncertain: {}", e);
            }
        }

        Ok(signature.to_string())
    }

    fn parse_commitment(&self) -> CommitmentConfig {
        match self.config.rpc_commitment.as_str() {
            "processed" => CommitmentConfig::processed(),
            "finalized" => CommitmentConfig::finalized(),
            _ => CommitmentConfig::confirmed(),
        }
    }

    pub async fn execute_with_flash_loan(
        &self,
        wallet: &Wallet,
        opp: &ArbitrageOpportunity,
        amount_usd: Decimal,
        submit: bool,
        rpc_url: &str,
        jito_client: Option<&JitoClient>,
    ) -> Result<TradeResult> {
        info!("⚡ Executing FLASH LOAN trade for opportunity: {}", opp.id);

        // 1. Resolve Mint
        let input_mint_str = self
            .token_map
            .get(&opp.pair.base)
            .ok_or_else(|| anyhow!("Unknown base token: {}", opp.pair.base))?;
        let input_mint = Pubkey::from_str(input_mint_str)?;

        // 2. Get Quote (Amount in atoms)
        let decimals = if opp.pair.base == "USDC" { 6 } else { 9 };
        let amount_atoms = (amount_usd * Decimal::from(10u64.pow(decimals)))
            .to_u64()
            .unwrap_or(0);

        // Use swap pair direction from opp?
        let quote = self
            .get_quote(&opp.pair.base, &opp.pair.base, amount_atoms)
            .await?;

        // 3. Get Swap Transaction (for instructions)
        let swap_req = SwapRequest {
            user_public_key: wallet.pubkey(), // Payer
            quote_response: quote.clone(),
            compute_unit_price_micro_lamports: None, // We set it in Builder
        };

        debug!("Requesting swap instructions (via transaction)...");
        let response = self
            .client
            .post(format!("{}/swap", JUPITER_API_URL))
            .json(&swap_req)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Jupiter swap request failed: {}",
                response.text().await?
            ));
        }

        let swap_resp: SwapResponse = response.json().await?;

        // 4. Extract Instructions
        let (swap_instructions, lookup_tables) = self
            .extract_instructions_from_tx(&swap_resp.swap_transaction)
            .await?;

        // 5. Build Flash Loan Tx
        let rpc_client_instance = RpcClient::new(rpc_url.to_string());
        let recent_blockhash = rpc_client_instance.get_latest_blockhash()?;

        // 5. Build Flash Loan Tx (V0)
        let tx = self
            .flash_loan_builder
            .build_transaction(
                opp,
                amount_atoms,
                &input_mint,
                swap_instructions,
                &lookup_tables,
                recent_blockhash,
            )
            .map_err(|e| anyhow!("Failed to build flash loan tx: {}", e))?;

        // 6. Submit
        let signature = if submit {
            // We need to sign and send.
            let client = RpcClient::new(rpc_url.to_string());
            let sig = client.send_and_confirm_transaction(&tx)?;
            sig.to_string()
        } else {
            "simulated_flash_loan_tx".to_string()
        };

        Ok(TradeResult {
            opportunity_id: opp.id,
            signature: Some(signature),
            success: true,                // Optimistic
            actual_profit: Decimal::ZERO, // Need to verify confirmation
            executed_at: chrono::Utc::now(),
            error: None,
        })
    }

    async fn extract_instructions_from_tx(
        &self,
        base64_tx: &str,
    ) -> Result<(
        Vec<solana_sdk::instruction::Instruction>,
        Vec<solana_sdk::address_lookup_table::AddressLookupTableAccount>,
    )> {
        let tx_bytes = BASE64_ENGINE.decode(base64_tx)?;
        let versioned_tx: VersionedTransaction = bincode::deserialize(&tx_bytes)?;
        let message = versioned_tx.message;

        match message {
            VersionedMessage::Legacy(msg) => {
                let instructions = msg
                    .instructions
                    .clone()
                    .into_iter()
                    .map(|ix| {
                        let program_id = msg.account_keys[ix.program_id_index as usize];
                        let accounts = ix
                            .accounts
                            .iter()
                            .map(|&idx| solana_sdk::instruction::AccountMeta {
                                pubkey: msg.account_keys[idx as usize],
                                is_signer: msg.is_signer(idx as usize),
                                is_writable: msg.is_writable(idx as usize),
                            })
                            .collect();

                        solana_sdk::instruction::Instruction {
                            program_id,
                            accounts,
                            data: ix.data.clone(),
                        }
                    })
                    .collect();
                Ok((instructions, vec![]))
            }
            VersionedMessage::V0(msg) => {
                if msg.address_table_lookups.is_empty() {
                    // No lookups. Use msg.account_keys directly.
                    // But V0 accounts are indices. Decompile logic needed here too?
                    // Yes, instructions use indices.
                    // Just use same manual logic but with empty dynamic parts.
                    let full_keys = &msg.account_keys;
                    let instructions = msg
                        .instructions
                        .iter()
                        .map(|ix| {
                            let program_id = full_keys[ix.program_id_index as usize];
                            let accounts = ix
                                .accounts
                                .iter()
                                .map(|&idx| {
                                    let idx = idx as usize;
                                    let pubkey = full_keys[idx];
                                    let is_signer =
                                        idx < msg.header.num_required_signatures as usize;
                                    let is_writable = if is_signer {
                                        idx < (msg.header.num_required_signatures
                                            - msg.header.num_readonly_signed_accounts)
                                            as usize
                                    } else {
                                        idx < (msg.account_keys.len()
                                            - msg.header.num_readonly_unsigned_accounts as usize)
                                    };
                                    solana_sdk::instruction::AccountMeta {
                                        pubkey,
                                        is_signer,
                                        is_writable,
                                    }
                                })
                                .collect();
                            solana_sdk::instruction::Instruction {
                                program_id,
                                accounts,
                                data: ix.data.clone(),
                            }
                        })
                        .collect();
                    Ok((instructions, vec![]))
                } else {
                    let alt_manager = self
                        .alt_manager
                        .as_ref()
                        .ok_or_else(|| anyhow!("ALTs required but AltManager not configured"))?;

                    let table_addresses: Vec<Pubkey> = msg
                        .address_table_lookups
                        .iter()
                        .map(|l| l.account_key)
                        .collect();
                    let tables = alt_manager.get_tables(&table_addresses).await?;

                    // Manual resolution since v0::Message might not expose it directly or correctly
                    let mut loaded_writable = Vec::new();
                    let mut loaded_readonly = Vec::new();

                    for lookup in &msg.address_table_lookups {
                        let table = tables
                            .iter()
                            .find(|t| t.key == lookup.account_key)
                            .ok_or_else(|| {
                                anyhow::anyhow!("Missing lookup table: {}", lookup.account_key)
                            })?;

                        for &idx in &lookup.writable_indexes {
                            let idx = idx as usize;
                            if idx < table.addresses.len() {
                                loaded_writable.push(table.addresses[idx]);
                            } else {
                                return Err(anyhow::anyhow!(
                                    "Lookup index {} out of bounds for table {}",
                                    idx,
                                    table.key
                                ));
                            }
                        }

                        for &idx in &lookup.readonly_indexes {
                            let idx = idx as usize;
                            if idx < table.addresses.len() {
                                loaded_readonly.push(table.addresses[idx]);
                            } else {
                                return Err(anyhow::anyhow!(
                                    "Lookup index {} out of bounds for table {}",
                                    idx,
                                    table.key
                                ));
                            }
                        }
                    }

                    let mut full_keys = msg.account_keys.clone();
                    full_keys.extend(loaded_writable.clone());
                    full_keys.extend(loaded_readonly.clone());

                    let static_len = msg.account_keys.len();
                    let writable_len = loaded_writable.len();

                    let instructions = msg
                        .instructions
                        .iter()
                        .map(|ix| {
                            let program_id_idx = ix.program_id_index as usize;
                            let program_id = full_keys[program_id_idx];

                            let accounts = ix
                                .accounts
                                .iter()
                                .map(|&idx| {
                                    let idx = idx as usize;
                                    let pubkey = full_keys[idx];

                                    let is_signer =
                                        idx < msg.header.num_required_signatures as usize;

                                    let is_writable = if idx < static_len {
                                        // Static account logic
                                        if is_signer {
                                            idx < (msg.header.num_required_signatures
                                                - msg.header.num_readonly_signed_accounts)
                                                as usize
                                        } else {
                                            idx < (static_len
                                                - msg.header.num_readonly_unsigned_accounts
                                                    as usize)
                                        }
                                    } else {
                                        // Dynamic account logic
                                        idx < (static_len + writable_len)
                                    };

                                    solana_sdk::instruction::AccountMeta {
                                        pubkey,
                                        is_signer,
                                        is_writable,
                                    }
                                })
                                .collect();

                            solana_sdk::instruction::Instruction {
                                program_id,
                                accounts,
                                data: ix.data.clone(),
                            }
                        })
                        .collect();

                    Ok((instructions, tables))
                }
            }
        }
    }
}
