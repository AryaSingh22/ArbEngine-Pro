use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::collections::HashMap;

/// Tracks volatility for different trading pairs using EWMA
pub struct VolatilityTracker {
    /// Map of pair symbol to current volatility (std dev estimate)
    volatilities: HashMap<String, Decimal>,
    /// Map of pair symbol to last price
    last_prices: HashMap<String, Decimal>,
    /// Decay factor for EWMA (lambda)
    decay: Decimal,
}

impl VolatilityTracker {
    pub fn new(window_size: usize) -> Self {
        // Calculate decay factor lambda = 2 / (N + 1)
        let n = Decimal::from(window_size);
        let decay = Decimal::from(2) / (n + Decimal::ONE);

        Self {
            volatilities: HashMap::new(),
            last_prices: HashMap::new(),
            decay,
        }
    }

    pub fn update_price(&mut self, pair: &str, price: Decimal) {
        if let Some(&last_price) = self.last_prices.get(pair) {
            // Calculate return: ln(price / last_price)
            // Approx: (price - last_price) / last_price
            let ret = (price - last_price) / last_price;
            let ret_sq = ret * ret;

            // Update variance using EWMA
            // Var_t = lambda * r_t^2 + (1 - lambda) * Var_{t-1}
            let current_vol_sq = self
                .volatilities
                .get(pair)
                .map(|v| v * v)
                .unwrap_or(Decimal::ZERO);

            let new_vol_sq = self.decay * ret_sq + (Decimal::ONE - self.decay) * current_vol_sq;

            // Store volatility (sqrt of variance)
            // Decimal doesn't have sqrt, convert to f64 and back
            if let Some(vol_sq_f64) = new_vol_sq.to_f64() {
                let vol = Decimal::try_from(vol_sq_f64.sqrt()).unwrap_or(Decimal::ZERO);
                self.volatilities.insert(pair.to_string(), vol);
            }
        }

        self.last_prices.insert(pair.to_string(), price);
    }

    pub fn get_volatility(&self, pair: &str) -> Option<Decimal> {
        self.volatilities.get(pair).cloned()
    }
}
