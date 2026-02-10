use crate::types::{ArbitrageOpportunity};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use chrono::Utc;

#[derive(Debug, Serialize, Deserialize)]
pub struct TradeRecord {
    pub timestamp: String,
    pub session_id: String,
    pub trade_type: String, // "SIMULATION" or "REAL"
    pub pair: String,
    pub buy_dex: String,
    pub sell_dex: String,
    pub size_usd: String,
    pub profit_usd: String,
    pub profit_pct: String,
    pub tx_signature: Option<String>,
    pub success: bool,
    pub error: Option<String>,
}

pub struct HistoryRecorder {
    file_path: String,
    session_id: String,
}

impl HistoryRecorder {
    pub fn new(file_path: &str, session_id: &str) -> Self {
        // Ensure directory exists
        if let Some(parent) = Path::new(file_path).parent() {
            let _ = fs::create_dir_all(parent);
        }

        Self {
            file_path: file_path.to_string(),
            session_id: session_id.to_string(),
        }
    }

    pub fn record_trade(
        &self,
        opp: &ArbitrageOpportunity,
        size_usd: Decimal,
        profit_usd: Decimal,
        success: bool,
        tx_sig: Option<String>,
        error: Option<String>,
        is_dry_run: bool,
    ) {
        let record = TradeRecord {
            timestamp: Utc::now().to_rfc3339(),
            session_id: self.session_id.clone(),
            trade_type: if is_dry_run { "SIMULATION".to_string() } else { "REAL".to_string() },
            pair: opp.pair.symbol(),
            buy_dex: opp.buy_dex.display_name().to_string(),
            sell_dex: opp.sell_dex.display_name().to_string(),
            size_usd: size_usd.round_dp(2).to_string(),
            profit_usd: profit_usd.round_dp(4).to_string(),
            profit_pct: opp.net_profit_pct.round_dp(2).to_string(),
            tx_signature: tx_sig,
            success,
            error,
        };

        match serde_json::to_string(&record) {
            Ok(json) => {
                 let open_result = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&self.file_path);
                
                match open_result {
                    Ok(mut file) => {
                         if let Err(e) = writeln!(file, "{}", json) {
                            eprintln!("Failed to write to history file: {}", e);
                        }
                    },
                    Err(e) => eprintln!("Failed to open history file {}: {}", self.file_path, e),
                }
            },
            Err(e) => eprintln!("Failed to serialize trade record: {}", e),
        }
    }
}
