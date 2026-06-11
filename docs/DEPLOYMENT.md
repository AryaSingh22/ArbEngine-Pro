# 💰 Ultimate Solana Arbitrage Deployment Guide

**Goal:** running a high-frequency trading bot to find price differences (arbitrage) on Solana and execute trades for profit.

---

## � Requirements (Before You Start)

To make money, you need money (liquidity) and fast internet.

1.  **Solana Wallet**: You need a dedicated wallet.
    - Create a new wallet (e.g., Phantom or generated via CLI).
    - **Export the Private Key** (Base58 format).
    - ⚠️ **Funding**: Load it with at least **2-5 SOL**.
        - 0.5 SOL for transaction fees (gas).
        - 1.5+ SOL for trading capital (USDC/SOL swaps).

2.  **Fast connection (RPC Node)**
    - The public Solana API is too slow for arbitrage. You will fail 99% of trades if you use it.
    - **Get a paid RPC**:
        - [Helius.xyz](https://helius.xyz) (Developer Plan is decent to start).
        - [Quicknode](https://quicknode.com).
    - You need the **HTTP URL** (e.g., `https://mainnet.helius-rpc.com/...`).

3.  **Server (VPS)** - *Highly Recommended*
    - Don't run this on your home WiFi laptop.
    - Rent a **Linux VPS** (Ubuntu 22.04).
    - **Provider**: AWS (US-East-1), DigitalOcean, or Vultr.
    - **Specs**: 4 vCPU, 8GB RAM minimum.

---

## 🛠️ Step-by-Step Installation

### Option A: The Easy Way (Docker)
*Works on Windows, Mac, and Linux.*

#### 1. Install Docker
- **Windows/Mac**: Download [Docker Desktop](https://www.docker.com/products/docker-desktop/).
- **Linux**: `sudo apt install docker.io docker-compose`

#### 2. Configure the Bot
Inside the project folder, rename `.env.example` to `.env` and open it with a text editor.

Fill in these **Critical Settings**:
```ini
# Your Paid RPC URL (Crucial for speed)
SOLANA_RPC_URL="https://your-helius-rpc-url..."

# Your Wallet Private Key (Base58 string)
# ⚠️ Keep this secret! Never share it.
PRIVATE_KEY="YOUR_PRIVATE_KEY_HERE"

# Risk Settings
# Minimum profit to trigger a trade (1.0 = 1%)
MIN_PROFIT_THRESHOLD=0.5 

# Auto-Trade Mode
# Set to 'false' to trade with REAL MONEY.
# Set to 'true' to just simulate and watch (recommended for first 24h).
DRY_RUN=false
```

#### 3. Launch Everything
Open your terminal/command prompt in the project folder and run:

```bash
docker-compose up --build -d
```

- This downloads necessary databases.
- Compiles the bot (takes ~5-10 mins).
- Starts the Dashboard.

#### 4. Open the Dashboard
Go to your browser: **[http://localhost:5173](http://localhost:5173)**

---

## ⚙️ Configuration Reference (new in this release)

### Jupiter API
The bot uses Jupiter's current Swap API (`swap/v1`) and Price API V3. The
legacy `quote-api.jup.ag/v6` and `price.jup.ag/v6` hosts were sunset by
Jupiter and no longer work.

```ini
# Optional: API key for the higher rate-limit api.jup.ag tier.
# Get one at https://portal.jup.ag. Without it the bot uses the free
# lite-api.jup.ag host (rate limited, fine for dry runs).
JUPITER_API_KEY=

# Optional overrides (defaults are chosen automatically from the key above)
JUPITER_SWAP_API_URL=
JUPITER_PRICE_API_URL=
```

### Jito dynamic tips
Bundles compete in a tip auction; a fixed tip stops landing exactly when
opportunities cluster. The bot bids a live percentile of recently landed tips
(from Jito's tip floor API), clamped between `JITO_TIP_LAMPORTS` (floor and
offline fallback) and `JITO_MAX_TIP_LAMPORTS`. On top of that, the tip is
capped at a fraction of the trade's expected profit (converted via the live
SOL price) — tipping more than the trade earns is a guaranteed loss.

```ini
USE_JITO=true
JITO_DYNAMIC_TIPS=true             # default true
JITO_TIP_PERCENTILE=75             # 25 / 50 / 75 / 95 / 99
JITO_TIP_LAMPORTS=10000            # minimum bid + fallback
JITO_MAX_TIP_LAMPORTS=1000000      # hard cap (0.001 SOL)
JITO_TIP_MAX_PROFIT_FRACTION=0.5   # never tip > 50% of expected profit
```

### Dynamic priority fees (non-Jito path)
For plain RPC submission the bot estimates the 75th percentile of recent
on-chain prioritization fees and bids that, clamped to
[`PRIORITY_FEE`, `PRIORITY_FEE` × 10]. Disable with
`DYNAMIC_PRIORITY_FEES=false` to always pay the static fee.

### Price sanity guard
With three or more sources for a pair, quotes deviating more than
`PRICE_SANITY_MAX_DEVIATION_PCT` (default 20%) from the per-pair median are
dropped before detection — one stale or misdecoded feed can no longer
fabricate phantom spreads.

### On-chain price streaming (Phase 2 data plane)
The old Raydium/Orca REST stats APIs served data that was minutes stale, so
they were removed from detection (`ENABLE_LEGACY_REST_PROVIDERS=true` restores
them — not recommended). Instead, the bot can stream AMM vault accounts over
the RPC WebSocket and compute pool prices locally, event-driven:

```ini
# Entries separated by ';'.
# Constant-product pools (Raydium AMM v4 etc.) — the two vault token accounts:
#   Dex:BASE-QUOTE:base_vault:quote_vault:base_decimals:quote_decimals
# Concentrated-liquidity pools — the pool account (sqrt price decoded locally;
# BASE must be the pool's token0/tokenA):
#   OrcaWhirlpool:BASE-QUOTE:pool_address:base_decimals:quote_decimals
#   RaydiumClmm:BASE-QUOTE:pool_address:base_decimals:quote_decimals
STREAMING_POOLS=Raydium:SOL-USDC:<base_vault>:<quote_vault>:9:6;OrcaWhirlpool:SOL-USDC:<whirlpool>:9:6

# Optional dedicated WebSocket endpoint (defaults to SOLANA_RPC_URL with wss://)
SOLANA_WS_URL=
```

### Measuring competitiveness
Two Prometheus metrics are the scoreboard — check them before tuning anything:
- `arb_tick_to_trade_seconds` — price tick → execution complete latency.
- `arb_jito_bundles_landed_total / arb_jito_bundles_submitted_total` — land rate.

---

## 📈 Improving Your Odds (How to actually profit)

Arbitrage is competitive. To win:

### 1. Reduce Latency
- Your bot competes with others to be the *first* to see a price difference.
- **Action**: Deploy your VPS in the **same region** as the RPC provider (usually US-East N.Virginia or Tokyo).
- **Action**: Configure `STREAMING_POOLS` so detection reacts to on-chain vault changes instead of waiting for the HTTP poll cycle.
- **Next level**: A Geyser/Yellowstone gRPC feed from your RPC provider (Helius, Triton, Chainstack) streams account writes at sub-50ms latency; pairing it with Jito ShredStream improves arrival times by ~120ms further. Both require a paid plan and replace the WebSocket streamer.

### 2. Pay Priority Fees / Tips
- When Solana is busy, cheap transactions fail.
- **Action**: Keep `JITO_DYNAMIC_TIPS=true` so bundle bids track the live auction. Raise `JITO_TIP_PERCENTILE` to 95 during volatile windows if your land rate drops.
- Non-Jito sends use `PRIORITY_FEE` (per compute unit) with Jupiter's dynamic compute-unit limit, so the fee budget prices correctly.

### 3. Start Small
- Keep `MAX_POSITION_SIZE` in `risk.rs` small (e.g., $50-$100) until you see consistent wins.
- Arbitrage is lower risk than trading memecoins, but technical bugs can still lose money (e.g., failed landing fees).

---

## ❓ Troubleshooting

**Q: I see "Docker Desktop is not running" error.**
A: Launch the Docker Desktop app on your computer first.

**Q: I see "Simulation" or "Dry Run" in logs.**
A: Change `DRY_RUN=false` in your `.env` file and restart (`docker-compose restart bot`).

**Q: I'm not getting any trades.**
A: 
1. Profit threshold might be too high (0.5% is hard to find instantly). Try 0.1% or 0.2%.
2. Your RPC is too slow (using public node).
3. The market is efficient right now. Wait for volatility.

**Q: Where are the logs?**
A: Run `docker-compose logs -f bot` to see the brain of the bot working.
