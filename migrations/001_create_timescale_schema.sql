-- Enable TimescaleDB extension
CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;

-- Price ticks table
CREATE TABLE price_ticks (
    time TIMESTAMPTZ NOT NULL,
    pair VARCHAR(20) NOT NULL,
    source VARCHAR(20) NOT NULL,
    price DOUBLE PRECISION NOT NULL,
    volume DOUBLE PRECISION,
    liquidity BIGINT
);

SELECT create_hypertable('price_ticks', 'time');
CREATE INDEX idx_price_pair_time ON price_ticks (pair, time DESC);

-- Opportunities table
CREATE TABLE opportunities (
    time TIMESTAMPTZ NOT NULL,
    opportunity_id UUID PRIMARY KEY,
    path TEXT NOT NULL,
    expected_profit_bps DOUBLE PRECISION NOT NULL,
    input_amount DOUBLE PRECISION NOT NULL,
    dex_route TEXT NOT NULL,
    status VARCHAR(20) NOT NULL  -- detected, executed, failed, expired
);

SELECT create_hypertable('opportunities', 'time');
CREATE INDEX idx_opp_status ON opportunities (status, time DESC);

-- Trades table
CREATE TABLE trades (
    time TIMESTAMPTZ NOT NULL,
    trade_id UUID PRIMARY KEY,
    opportunity_id UUID REFERENCES opportunities(opportunity_id),
    signature VARCHAR(100) NOT NULL,
    actual_profit DOUBLE PRECISION,
    execution_time_ms INTEGER,
    slippage_bps DOUBLE PRECISION,
    gas_used BIGINT,
    priority_fee BIGINT,
    status VARCHAR(20) NOT NULL  -- success, failed, timeout
);

SELECT create_hypertable('trades', 'time');
CREATE INDEX idx_trades_signature ON trades (signature);

-- Performance metrics table
CREATE TABLE performance_metrics (
    time TIMESTAMPTZ NOT NULL,
    metric_name VARCHAR(50) NOT NULL,
    metric_value DOUBLE PRECISION NOT NULL,
    tags JSONB
);

SELECT create_hypertable('performance_metrics', 'time');
CREATE INDEX idx_metrics_name ON performance_metrics (metric_name, time DESC);

-- Continuous aggregates for fast queries
CREATE MATERIALIZED VIEW hourly_profits
WITH (timescaledb.continuous) AS
SELECT time_bucket('1 hour', time) AS bucket,
       COUNT(*) as trade_count,
       SUM(actual_profit) as total_profit,
       AVG(actual_profit) as avg_profit,
       MAX(actual_profit) as max_profit,
       MIN(actual_profit) as min_profit,
       AVG(slippage_bps) as avg_slippage
FROM trades
WHERE status = 'success'
GROUP BY bucket;

SELECT add_continuous_aggregate_policy('hourly_profits',
    start_offset => INTERVAL '3 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour');
