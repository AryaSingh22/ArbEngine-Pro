use anyhow::{anyhow, Result};
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    instruction::Instruction,
    nonce::{state::Versions as NonceVersions, State as NonceState},
    pubkey::Pubkey,
    system_instruction,
};

/// Fetch the nonce account state and extract the durable nonce (blockhash).
pub async fn fetch_nonce_account(client: &RpcClient, nonce_pubkey: &Pubkey) -> Result<String> {
    let account = client.get_account(nonce_pubkey).await?;
    
    // Parse the nonce state
    let nonce_state: NonceVersions = bincode::deserialize(&account.data)
        .map_err(|e| anyhow!("Failed to deserialize nonce account data: {}", e))?;
        
    match nonce_state.state() {
        NonceState::Initialized(data) => Ok(data.blockhash().to_string()),
        _ => Err(anyhow!("Nonce account is not initialized")),
    }
}

/// Create an instruction to advance the durable nonce.
pub fn create_advance_nonce_instruction(
    nonce_pubkey: &Pubkey,
    authorized_pubkey: &Pubkey,
) -> Instruction {
    system_instruction::advance_nonce_account(nonce_pubkey, authorized_pubkey)
}
