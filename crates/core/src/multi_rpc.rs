use std::time::Duration;
use std::sync::Arc;
use futures::future::join_all;
use anyhow::{Result, anyhow};
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_rpc_client_api::config::RpcSendTransactionConfig;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::Signature,
    transaction::VersionedTransaction,
    hash::Hash,
    account::Account,
};
use tracing::{info, warn, debug};

/// A multi-client RPC wrapper providing redundancy and broadcast capabilities
pub struct MultiRpcClient {
    pub clients: Vec<Arc<RpcClient>>,
}

impl MultiRpcClient {
    /// Create a new MultiRpcClient from a list of URLs
    pub fn new(urls: Vec<String>, timeout_ms: u64, commitment: CommitmentConfig) -> Self {
        let timeout = Duration::from_millis(timeout_ms);
        let clients = if urls.is_empty() {
            tracing::warn!("No RPC URLs provided to MultiRpcClient, falling back to localhost");
            vec![Arc::new(RpcClient::new_with_timeout_and_commitment(
                "http://localhost:8899".to_string(),
                timeout,
                commitment,
            ))]
        } else {
            urls.into_iter()
                .map(|url| {
                    Arc::new(RpcClient::new_with_timeout_and_commitment(
                        url, timeout, commitment,
                    ))
                })
                .collect()
        };
        
        Self { clients }
    }

    /// Broadcast a transaction to all configured RPC nodes simultaneously
    pub async fn send_transaction(&self, tx: &VersionedTransaction, config: RpcSendTransactionConfig) -> Result<Signature> {
        let futures = self.clients.iter().map(|client| {
            let client = client.clone();
            let tx = tx.clone();
            tokio::spawn(async move {
                client.send_transaction_with_config(&tx, config).await
            })
        });

        let results: Vec<_> = join_all(futures).await;

        let mut first_success = None;
        let mut errors = Vec::new();

        for (idx, res) in results.into_iter().enumerate() {
            match res {
                Ok(Ok(sig)) => {
                    info!("✅ RPC #{} successfully broadcast transaction: {}", idx, sig);
                    if first_success.is_none() {
                        first_success = Some(sig);
                    }
                }
                Ok(Err(e)) => {
                    warn!("⚠️ RPC #{} failed to broadcast: {}", idx, e);
                    errors.push(e.to_string());
                }
                Err(e) => {
                    warn!("⚠️ RPC #{} task panicked or failed: {}", idx, e);
                    errors.push(e.to_string());
                }
            }
        }

        first_success.ok_or_else(|| anyhow!("All RPC clients failed to send transaction: {:?}", errors))
    }

    /// Implement a fallback strategy for fetching account data
    pub async fn get_account(&self, pubkey: &Pubkey) -> Result<Account> {
        for (idx, client) in self.clients.iter().enumerate() {
            match client.get_account(pubkey).await {
                Ok(acc) => return Ok(acc),
                Err(e) => {
                    debug!("RPC #{} failed to get account {}: {}", idx, pubkey, e);
                }
            }
        }
        Err(anyhow!("All RPC clients failed to get account {}", pubkey))
    }

    /// Implement a fallback strategy for getting the latest blockhash
    pub async fn get_latest_blockhash(&self) -> Result<Hash> {
        for (idx, client) in self.clients.iter().enumerate() {
            match client.get_latest_blockhash().await {
                Ok(hash) => return Ok(hash),
                Err(e) => {
                    debug!("RPC #{} failed to get blockhash: {}", idx, e);
                }
            }
        }
        Err(anyhow!("All RPC clients failed to get latest blockhash"))
    }
}
