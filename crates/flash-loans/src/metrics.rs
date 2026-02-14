use rust_decimal::Decimal;

// Placeholder for now. In a full implementation, we'd define specific metrics here
// similar to the bot's MetricsCollector.

pub struct FlashLoanMetrics {
    pub total_borrowed_volume: Decimal,
    pub total_fees_paid: Decimal,
    pub success_count: u64,
    pub failure_count: u64,
}

impl Default for FlashLoanMetrics {
    fn default() -> Self {
        Self {
            total_borrowed_volume: Decimal::ZERO,
            total_fees_paid: Decimal::ZERO,
            success_count: 0,
            failure_count: 0,
        }
    }
}
