
use solana_arb_core::{
    dex::DexProvider,
    error::ArbitrageError,
    types::{DexType, PriceData, TokenPair},
    ArbitrageResult,
};
use tokio::sync::mpsc;

pub struct PhoenixProvider {
    // Placeholder
}

impl Default for PhoenixProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl PhoenixProvider {
    pub fn new() -> Self {
        Self {}
    }
}

impl DexProvider for PhoenixProvider {
    fn dex_type(&self) -> DexType {
        DexType::Phoenix
    }

    fn get_price<'a>(&'a self, _pair: &'a TokenPair) -> solana_arb_core::dex::BoxFuture<'a, ArbitrageResult<PriceData>> {
        Box::pin(async move {
            Err(ArbitrageError::PriceFetch(
                "Phoenix price fetching not implemented".to_string(),
            ))
        })
    }

    fn subscribe<'a>(
        &'a self,
        _pairs: Vec<TokenPair>,
    ) -> solana_arb_core::dex::BoxFuture<'a, ArbitrageResult<mpsc::Receiver<PriceData>>> {
        Box::pin(async move {
            Err(ArbitrageError::PriceFetch(
                "Phoenix subscription not implemented".to_string(),
            ))
        })
    }

    fn health_check<'a>(&'a self) -> solana_arb_core::dex::BoxFuture<'a, ArbitrageResult<bool>> {
        Box::pin(async move {
            Ok(true)
        })
    }
}
