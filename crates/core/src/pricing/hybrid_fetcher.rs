use crate::pricing::parallel_fetcher::ParallelPriceFetcher;
use crate::streaming::ws_manager::WebSocketManager;
use crate::types::{PriceData, TokenPair};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct HybridPriceFetcher {
    ws_manager: WebSocketManager,
    http_fetcher: ParallelPriceFetcher,
    // precise cache of latest prices
    price_cache: Arc<RwLock<HashMap<String, PriceData>>>,
}

impl HybridPriceFetcher {
    pub fn new(http_fetcher: ParallelPriceFetcher, ws_manager: WebSocketManager) -> Self {
        Self {
            ws_manager,
            http_fetcher,
            price_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn start(&self, pairs: &[TokenPair]) {
        // Start WebSocket subscriptions
        // Note: In a real implementation, we would determine which DEXs support WS
        // For now, we just log
        tracing::info!("Starting Hybrid Price Fetcher...");

        // Example: Subscribe to WS if available (logic inside ws_manager)
        // for pair in pairs {
        //     self.ws_manager.subscribe_to_pair(DexType::Jupiter, pair.clone()).await;
        // }
    }

    pub async fn fetch_all_prices(&self, pairs: &[TokenPair]) -> Vec<PriceData> {
        // 1. Fetch from HTTP (polling) - always reliable fallback
        let mut prices = self.http_fetcher.fetch_all_prices(pairs).await;

        // 2. Update cache with HTTP prices
        {
            let mut cache = self.price_cache.write().await;
            for price in &prices {
                cache.insert(price.pair.symbol(), price.clone());
            }
        }

        // 3. Return combined prices (HTTP + latest WS updates from cache)
        // Note: Since WS updates would update the cache asynchronously,
        // we might want to return the cached values if they are newer.
        // For now, we just return the HTTP fetched prices as the base,
        // relying on the fact that HTTP is the "tick".
        // A better approach is to return the cache content filtered by the requested pairs.

        prices
    }

    pub async fn get_price(&self, pair_symbol: &str) -> Option<PriceData> {
        let cache = self.price_cache.read().await;
        cache.get(pair_symbol).cloned()
    }
}
