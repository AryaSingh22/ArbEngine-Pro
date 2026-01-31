-- Initial schema for Solana Arbitrage Dashboard
-- Uses TimescaleDB for time-series data

-- Enable TimescaleDB extension
CREATE EXTENSION IF NOT EXISTS timescaledb;

-- Price data table (time-series)
CREATE TABLE IF NOT EXISTS price_data (
    id BIGSERIAL,
    dex VARCHAR(20) NOT NULL,
    base_token VARCHAR(50) NOT NULL,
    quote_token VARCHAR(50) NOT NULL,
    bid_price DECIMAL(30, 18) NOT NULL,
    ask_price DECIMAL(30, 18) NOT NULL,
    mid_price DECIMAL(30, 18) NOT NULL,
    volume_24h DECIMAL(30, 10),
    liquidity DECIMAL(30, 10),
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (id, timestamp)
);

-- Convert to hypertable for time-series optimization
SELECT create_hypertable('price_data', 'timestamp', if_not_exists => TRUE);

-- Create index for faster pair lookups
CREATE INDEX IF NOT EXISTS idx_price_data_pair ON price_data (base_token, quote_token, dex, timestamp DESC);

-- Arbitrage opportunities table (time-series)
CREATE TABLE IF NOT EXISTS opportunities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    base_token VARCHAR(50) NOT NULL,
    quote_token VARCHAR(50) NOT NULL,
    buy_dex VARCHAR(20) NOT NULL,
    sell_dex VARCHAR(20) NOT NULL,
    buy_price DECIMAL(30, 18) NOT NULL,
    sell_price DECIMAL(30, 18) NOT NULL,
    gross_profit_pct DECIMAL(10, 6) NOT NULL,
    net_profit_pct DECIMAL(10, 6) NOT NULL,
    estimated_profit_usd DECIMAL(20, 6),
    detected_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expired_at TIMESTAMPTZ
);

-- Create hypertable for opportunities
SELECT create_hypertable('opportunities', 'detected_at', if_not_exists => TRUE);

-- Index for finding active opportunities
CREATE INDEX IF NOT EXISTS idx_opportunities_active ON opportunities (expired_at) WHERE expired_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_opportunities_pair ON opportunities (base_token, quote_token, detected_at DESC);
CREATE INDEX IF NOT EXISTS idx_opportunities_profit ON opportunities (net_profit_pct DESC, detected_at DESC);

-- Trade execution history
CREATE TABLE IF NOT EXISTS trade_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    opportunity_id UUID NOT NULL,
    signature VARCHAR(100),
    success BOOLEAN NOT NULL DEFAULT FALSE,
    actual_profit DECIMAL(20, 10),
    gas_used DECIMAL(20, 10),
    error_message TEXT,
    executed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create hypertable for trade history
SELECT create_hypertable('trade_history', 'executed_at', if_not_exists => TRUE);

-- System configuration table
CREATE TABLE IF NOT EXISTS config (
    key VARCHAR(100) PRIMARY KEY,
    value JSONB NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Insert default configuration
INSERT INTO config (key, value) VALUES
    ('min_profit_threshold', '{"value": 0.5, "unit": "percent"}'::jsonb),
    ('max_position_size', '{"value": 1000, "unit": "usd"}'::jsonb),
    ('slippage_tolerance', '{"value": 1.0, "unit": "percent"}'::jsonb),
    ('monitored_pairs', '["SOL/USDC", "SOL/USDT", "RAY/USDC", "BONK/SOL", "JUP/USDC"]'::jsonb)
ON CONFLICT (key) DO NOTHING;

-- Retention policy: Keep detailed data for 90 days
SELECT add_retention_policy('price_data', INTERVAL '90 days', if_not_exists => TRUE);
SELECT add_retention_policy('opportunities', INTERVAL '90 days', if_not_exists => TRUE);
SELECT add_retention_policy('trade_history', INTERVAL '365 days', if_not_exists => TRUE);

-- Compression policy for older data
SELECT add_compression_policy('price_data', INTERVAL '7 days', if_not_exists => TRUE);
SELECT add_compression_policy('opportunities', INTERVAL '7 days', if_not_exists => TRUE);
