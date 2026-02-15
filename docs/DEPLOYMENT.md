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

## 📈 Improving Your Odds (How to actually profit)

Arbitrage is competitive. To win:

### 1. Reduce Latency
- Your bot competes with others to be the *first* to see a price difference.
- **Action**: Deploy your VPS in the **same region** as the RPC provider (usually US-East N.Virginia or Tokyo).

### 2. Pay Priority Fees
- When Solana is busy, cheap transactions fail.
- **Action**: The bot logic simulates fees. For real trading, you may need to tweak `execution.rs` to add a dynamic "bribe" (Compute Unit Price) to validators.

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
