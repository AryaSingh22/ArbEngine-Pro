use crate::types::{DexType, PriceData, TokenPair};
use futures_util::{SinkExt, StreamExt};
use rust_decimal::Decimal;
use serde_json::json;
use std::collections::HashMap;
use std::str::FromStr;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub struct WebSocketManager {
    price_tx: mpsc::Sender<PriceData>,
}

impl WebSocketManager {
    pub fn new(price_tx: mpsc::Sender<PriceData>) -> Self {
        Self { price_tx }
    }

    pub async fn subscribe_to_pair(&self, dex: DexType, pair: TokenPair) {
        let url = match dex {
            DexType::Jupiter => "wss://quote-api.jup.ag/v6/quote-ws".to_string(), // Example URL
            DexType::Raydium => format!("wss://api.raydium.io/v2/main/price/{}", pair.symbol()), // Example URL
            _ => return,
        };

        // This is a simplified implementation. Real WS connection needs reconnection logic, ping/pong, etc.
        let result = connect_async(&url).await;

        match result {
            Ok((ws_stream, _)) => {
                tracing::info!("🔌 Connected to WS for {} on {:?}", pair, dex);
                let (mut write, mut read) = ws_stream.split();

                // Send subscribe message if needed
                let subscribe_msg = json!({
                    "method": "subscribe",
                    "params": [pair.symbol()]
                });
                if let Err(e) = write.send(Message::Text(subscribe_msg.to_string())).await {
                    tracing::error!("Failed to send subscribe message: {}", e);
                    return;
                }

                let price_tx = self.price_tx.clone();
                let pair_clone = pair.clone(); // Clone for closure

                tokio::spawn(async move {
                    while let Some(Ok(msg)) = read.next().await {
                        if let Message::Text(text) = msg {
                            // Dummy parsing logic - needs to be adapted to specific DEX WS format
                            // This is a placeholder to show structure
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                                // Extract price...
                                // let price = ...;
                                // let price_data = PriceData::new(dex, pair_clone.clone(), bid, ask);
                                // let _ = price_tx.send(price_data).await;
                            }
                        }
                    }
                    tracing::warn!("WS disconnected for {} on {:?}", pair_clone, dex);
                });
            }
            Err(e) => {
                tracing::warn!("Failed to connect to WS for {} on {:?}: {}", pair, dex, e);
            }
        }
    }
}
