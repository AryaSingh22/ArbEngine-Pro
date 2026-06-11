# System Architecture & Internals

This document details the internal logic of the Solana Arbitrage Bot, specifically the arbitrage detection engine and risk management system.

## 1. Triangular Arbitrage Engine (`pathfinder.rs`)

The bot isn't limited to simple A->B->A swaps. It uses a **Graph-Based Depth-First Search (DFS)** to find profitable multi-hop paths.

### How it works:
1.  **Graph Construction**: 
    - Vertices = Tokens (SOL, USDC, RAY, etc.)
    - Edges = Active DEX Pools (Raydium, Orca, Jupiter)
    - Weights = Price / Exchange Rate

2.  **Path Discovery**:
    - The `PathFinder` explores paths starting from a *quote token* (e.g., USDC).
    - It recurses up to `MAX_HOPS` (default: 3) to find a cycle back to the start token.
    - **Example Path**: `USDC` -> `SOL` (Raydium) -> `RAY` (Orca) -> `USDC` (Jupiter).

3.  **Profit Calculation**:
    - Calculating the cumulative product of exchange rates along the path.
    - Deducting estimated fees (network fee + DEX trading fees).
    - `Net Profit % = (Final Amount / Initial Amount) - 1`

## 2. Risk Management (`risk.rs`)

To prevent catastrophic losses, the bot implements a robust `RiskManager` that acts as a middleware before any trade execution.

### Components:

#### A. Position Sizing (Kelly Criterion - Simplified)
Instead of betting the entire wallet, the bot calculates an optimal size based on:
- **Confidence**: `Profit %` (Higher profit = larger size)
- **Limits**: Never exceeds `max_position_size` config.

#### B. Circuit Breakers
- **Daily Loss Limit**: If `daily_pnl` drops below `-$100` (configurable), the bot enters a **PAUSED** state.
- **Cooldowns**: After a failed trade, specific pairs are blacklisted for a short duration to avoid repeating mistakes against toxic flow.

#### C. Exposure Limits
- **Max Total Exposure**: Caps the total USD value of all open trades (relevant for future async execution).

## 3. Bot Lifecycle (`bot/src/lib.rs`)

The `run_trading_loop` functions as the heartbeat:
1.  **Poll (`POLL_INTERVAL_MS`, default 500ms)**: Fetch latest prices (Jupiter Price API V3).
2.  **Merge streamed prices**: Event-driven on-chain prices (see §4) override HTTP prices for the same (DEX, pair).
3.  **Detect**: Run `ArbitrageDetector` (Simple) and `PathFinder` (Triangular).
4.  **Evaluate**: Pass best opportunity to `RiskManager`.
5.  **Execute**:
    - **Dry Run**: Log outcome, simulate profit/loss.
    - **Live**: Request quote & swap instructions from Jupiter Swap API (`swap/v1`), sign, and submit — via a Jito bundle carrying a dynamic tip (live landed-tip percentile from the tip floor API) or via multi-RPC broadcast with a priority fee.

## 4. On-Chain Account Streaming (`core/src/streaming/account_stream.rs`)

When `STREAMING_POOLS` is configured, the bot subscribes to pool state over
the Solana WebSocket `accountSubscribe` API and computes prices locally:
- **Constant-product pools** (Raydium AMM v4 etc.): both vault token
  accounts; spot price = decimal-adjusted reserve ratio.
- **Concentrated-liquidity pools** (Orca Whirlpool, Raydium CLMM): the pool
  account itself; price decoded from its Q64.64 sqrt price.

Prices are emitted the moment a pool changes — no aggregator round-trip —
and land in a shared cache that the detection loop merges each tick. A
sanity guard (`core/src/pricing/sanity.rs`) then drops quotes deviating more
than `PRICE_SANITY_MAX_DEVIATION_PCT` from the per-pair median, so one bad
feed cannot fabricate phantom spreads.

The legacy Raydium/Orca REST providers were removed from detection because
those endpoints serve cached aggregate stats (minutes stale), which produced
phantom spreads. Yellowstone gRPC (Geyser) is the planned lower-latency
replacement for the WebSocket transport.

## 5. Execution Scoreboard

Two Prometheus metrics measure competitiveness end-to-end:
- `arb_tick_to_trade_seconds` — price tick start → execution complete.
- `arb_jito_bundles_landed_total / arb_jito_bundles_submitted_total` — bundle land rate.
