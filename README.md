# ArbEngine Pro ⚡

**A production-grade, event-driven arbitrage trading engine for Solana, written in Rust.**

ArbEngine Pro detects and executes cross-DEX arbitrage with an on-chain streaming data plane, auction-aware Jito bundle submission, flash-loan capital efficiency, and multi-tier risk management — instrumented end-to-end with Prometheus metrics.

> Built on the Agave-era Solana SDK (2.x) and Jupiter's current Swap API (`swap/v1`) / Price API V3. The sunset `quote-api.jup.ag/v6` and `price.jup.ag` endpoints are fully migrated off.

---

## ✨ Feature Overview

### 📡 Event-Driven Data Plane
- **On-chain account streaming** — subscribes to pool state over Solana WebSocket `accountSubscribe` and computes prices **locally**, the moment a pool changes. No aggregator round-trips, no stale stats APIs.
  - **Constant-product pools** (Raydium AMM v4 style): price from decimal-adjusted vault reserve ratios.
  - **Concentrated-liquidity pools** (Orca Whirlpool, Raydium CLMM): price decoded directly from the pool's Q64.64 sqrt price.
- **Jupiter Price API V3** as the HTTP baseline feed (free `lite-api` tier or keyed `api.jup.ag` tier via `JUPITER_API_KEY`).
- **Price sanity guard** — with 3+ sources per pair, quotes deviating beyond a configurable percentage from the per-pair median are dropped, so one bad feed can't fabricate phantom spreads.
- Architecture is transport-agnostic: the WebSocket streamer is designed to be swapped for **Yellowstone gRPC (Geyser)** for sub-50ms feeds on serious infrastructure.

### 🚀 Execution Engine
- **Jupiter Swap API (`swap/v1`)** for quotes, swap transactions, and structured swap instructions, with `dynamicComputeUnitLimit` and liquid-route restriction for better landing rates.
- **Jito bundle submission** with **dynamic tips**: bids a live percentile of recently landed tips (Jito tip floor API, cached), clamped between a floor and a hard cap — static tips stop clearing the auction exactly when opportunities cluster.
- **Profit-aware tip capping**: never tips more than a configurable fraction of the trade's expected profit (converted via a live cached SOL price). Tipping more than a trade earns is a guaranteed loss.
- **Dynamic priority fees** (non-Jito path): bids the 75th percentile of recent on-chain prioritization fees, clamped to `[PRIORITY_FEE, 10×]`.
- **Flash loans** (Solend) — borrow, multi-hop swap, repay, all atomic in one transaction.
- **V0 transactions + Address Lookup Tables (ALTs)** for complex multi-hop bundles.
- **Durable nonces** for replay-safe offline-built transactions.
- **Multi-RPC broadcast** — fires transactions at every configured RPC simultaneously and takes the first success, with Jito → RPC fallback.

### 🧠 Strategy & Detection
- **Simple cross-DEX detector** plus **graph-based DFS pathfinder** for triangular/multi-hop cycles (up to 4 hops).
- **Statistical arbitrage** (z-score mean reversion over a rolling window).
- **Latency arbitrage** strategy for cross-DEX update lag.
- Pluggable `Strategy` trait — drop in new alpha without touching the loop.

### 🛡️ Risk Management
- **Circuit breakers** — auto-pause on daily loss limits or consecutive failures, with timed cool-down and event-bus escalation.
- **Position sizing** — simplified Kelly with hard caps and per-pair cooldowns after losses.
- **Exposure limits & VaR monitoring** in real time.
- **Pre-flight safety checks** at boot (RPC health, wallet, config validation) and a file-based kill switch (`.kill`) for instant shutdown.
- **Dry-run by default** — `DRY_RUN=true` simulates everything; live trading is opt-in.

### 📊 Observability
- **Prometheus metrics** on `:9090/metrics`, including the two numbers that actually measure competitiveness:
  - `arb_tick_to_trade_seconds` — price tick → execution complete latency.
  - `arb_jito_bundles_landed_total / arb_jito_bundles_submitted_total` — bundle land rate.
  - Plus opportunities, trade outcomes, profit distribution, fetch latency, slippage, balance, circuit-breaker state.
- **Health endpoint** on `:8080`, trade history journals (JSONL), structured `tracing` logs, and Telegram/Discord alerting hooks.
- **React dashboard** (React 19 + Vite + Recharts) for live visualization.

---

## 🏗️ Architecture

```
                ┌────────────────────────────────────────────────┐
                │                  DATA PLANE                    │
   WebSocket ──▶│  AccountStreamer (CPMM vaults / CLMM sqrt px)  │
   Jupiter   ──▶│  Price API V3 baseline feed                    │
                │            ▼ price sanity guard                │
                └───────────────────┬────────────────────────────┘
                                    ▼
                ┌────────────────────────────────────────────────┐
                │                 DETECTION                      │
                │  ArbitrageDetector · PathFinder (DFS, 4 hops)  │
                │  Statistical & Latency strategies              │
                └───────────────────┬────────────────────────────┘
                                    ▼
                ┌────────────────────────────────────────────────┐
                │                RISK GATE                       │
                │  Kelly sizing · circuit breakers · VaR · caps  │
                └───────────────────┬────────────────────────────┘
                                    ▼
                ┌────────────────────────────────────────────────┐
                │                EXECUTION                       │
                │  Jupiter swap/v1 → (flash loan wrap) →         │
                │  Jito bundle (dynamic, profit-capped tip)      │
                │  or multi-RPC broadcast (dynamic priority fee) │
                └────────────────────────────────────────────────┘
```

### Workspace Layout

```
crates/
├── bot/            # Main binary: trading loop, executor, wallet, alerts, metrics
├── core/           # Shared engine: detection, pathfinding, risk, streaming,
│                   # pricing, Jito client, multi-RPC, nonces, ALTs, config
├── api/            # Optional WebSocket API server for the dashboard
├── flash-loans/    # Solend flash loan provider + safety checks
├── dex-plugins/    # Additional DEX connectors (Lifinity, Meteora, Phoenix)
└── strategies/     # Statistical & latency arbitrage strategies
dashboard/          # React 19 + Vite + Recharts frontend
config/             # trading_config.json (hot-reload), solend_reserves.json
docs/               # DEPLOYMENT.md, INTERNALS.md
```

---

## 🚀 Quick Start

### Prerequisites
- Rust 1.75+
- A **paid RPC provider** for live trading (Helius, QuickNode, Triton — the public endpoint is fine for dry runs only)
- Docker (optional, for the full stack with dashboard)

### 1. Clone & configure

```bash
git clone https://github.com/AryaSingh22/ArbEngine-Pro.git
cd ArbEngine-Pro
cp .env.example .env   # then edit — every variable is documented inline
```

### 2. Build & run (dry run — default)

```bash
cargo build -p solana-arb-bot --release

# Windows
./target/release/bot.exe

# Linux/Mac
./target/release/bot
```

The bot boots in **simulation mode** (`DRY_RUN=true`), passes pre-flight checks, and starts detecting against live market data without risking funds. Health: `http://localhost:8080` · Metrics: `http://localhost:9090/metrics`.

### 3. Docker (full stack)

```bash
docker-compose up --build -d   # bot + database + dashboard
```

### 4. Go live (when ready)

Set in `.env`: `DRY_RUN=false`, a funded `PRIVATE_KEY`, a paid `SOLANA_RPC_URL`, and ideally `USE_JITO=true` + `JUPITER_API_KEY`. Start small.

---

## ⚙️ Key Configuration

All settings live in `.env` (see [.env.example](.env.example) for the complete annotated list) plus hot-reloadable trading parameters in [config/trading_config.json](config/trading_config.json).

| Variable | Purpose | Default |
| :--- | :--- | :--- |
| `DRY_RUN` | Simulate instead of trading | `true` |
| `SOLANA_RPC_URL` / `RPC_URLS` | Primary RPC / redundant multi-RPC list | public |
| `JUPITER_API_KEY` | Keyed api.jup.ag tier (higher rate limits) | free tier |
| `STREAMING_POOLS` | On-chain pools to stream (CPMM + CLMM formats) | off |
| `USE_JITO` | Submit via Jito bundles | `false` |
| `JITO_DYNAMIC_TIPS` / `JITO_TIP_PERCENTILE` | Live tip-auction bidding | `true` / `75` |
| `JITO_TIP_MAX_PROFIT_FRACTION` | Max tip as fraction of expected profit | `0.5` |
| `DYNAMIC_PRIORITY_FEES` | p75 on-chain fee estimation (non-Jito) | `true` |
| `PRICE_SANITY_MAX_DEVIATION_PCT` | Outlier quote filter threshold | `20` |
| `MIN_PROFIT_THRESHOLD` / `SLIPPAGE_BPS` | Trade gating | `0.5` / `50` |
| `ENABLE_FLASH_LOANS` | Solend flash-loan execution | `false` |
| `NONCE_ACCOUNT_PUBKEYS` | Durable nonce accounts | — |

---

## 🧪 Quality

- **117 automated tests** across all six crates — detection math, risk escalation, circuit breakers, rate-limiter recovery, config validation, streaming decoders (SPL/CLMM layouts), price sanity filtering, Jupiter instruction conversion.
- **Zero clippy warnings** across the workspace (`cargo clippy --workspace --all-targets`).
- Verified end-to-end: release build → full test suite → live dry-run boot against mainnet RPC.

---

## 📚 Documentation

- **[Deployment Guide](docs/DEPLOYMENT.md)** — VPS/Docker setup, configuration reference, profit-odds tuning, troubleshooting.
- **[Internals](docs/INTERNALS.md)** — pathfinding engine, risk system, streaming data plane, execution scoreboard.

## 🗺️ Roadmap

- **Yellowstone gRPC (Geyser)** transport for the streaming data plane (sub-50ms account feeds).
- **Full CLMM depth math** (tick-array traversal) for sized quotes, not just spot.
- **On-chain atomic arbitrage program** with an on-chain profit check (abort-if-unprofitable).
- **ShredStream pairing + colocated deployment** for infrastructure-level latency wins.

## ⚠️ Disclaimer

This software is for educational purposes. Cryptocurrency trading involves substantial risk; arbitrage is highly competitive and profitability is never guaranteed. The authors are not responsible for any financial losses incurred while using this software. Use at your own risk — and start in `DRY_RUN`.

## License

MIT
