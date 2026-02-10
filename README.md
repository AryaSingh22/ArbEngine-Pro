# Solana Arbitrage Dashboard & Trading Bot
 
A high-performance arbitrage opportunity detection and automated trading system for Solana DEXs, built with Rust + Myers-Diff logic. Now **Production Ready** 🚀.

## ✅ Features

### Phase 1: Core & Dashboard
- **Real-time Price Monitoring** - 500ms polling (Raydium, Orca, Jupiter)
- **Arbitrage Detection** - Automatic opportunity identification via Bellman-Ford / DFS
- **Dashboard** - React-based UI for monitoring opportunities (currently optional)

### Phase 2: Trading Engine
- **Triangular Arbitrage** - Multi-hop path discovery (e.g., SOL -> USDC -> BONK -> SOL)
- **Risk Management** - Circuit breakers, position limits, and daily loss caps
- **Dry-Run Mode** - Safe testing simulated environment

### Phase 3: Production Readiness (NEW)
- **🛡️ Jito MEV Protection** - Bundle submission to bypass public mempool and avoid sandwich attacks.
- **⚡ Priority Fees** - Dynamic compute unit pricing (`PRIORITY_FEE`) to land transactions during congestion.
- **🎯 Dynamic Slippage** - Configurable basis points (`SLIPPAGE_BPS`) for trade execution.
- **🔄 Retry Logic** - Exponential backoff for failed transactions.
- **💰 Balance Guards** - Pre-trade solvency checks.

## 🚀 Quick Start

### Prerequisites
- Rust (latest stable)
- Solana CLI tools
- Paid RPC Provider (Helius, QuickNode, Triton) for live trading

### Build the Bot
> **Note:** The bot is built as a standalone crate to ensure dependency stability.

```bash
# Build release binary
cargo build -p solana-arb-bot --release
```

### Configuration
1. Copy the example config:
   ```bash
   cp .env.example .env
   ```
2. Edit `.env` with your keys:
   - `PRIVATE_KEY` (Base58)
   - `SOLANA_RPC_URL` (HTTPS)
   - `USE_JITO=true` (Optional)

### Run
```bash
# Run in Dry-Run Mode (Safe)
cargo run -p solana-arb-bot

# Run in Production (Live)
# Ensure DRY_RUN=false in .env
./target/release/bot
```

## 🏗️ Architecture

The system uses a modular architecture optimized for speed and reliability.

```mermaid
flowchart LR
    subgraph DEXs["Solana Ecosystem"]
        J[Jupiter Aggregator]
        R[Raydium]
        O[Orca]
        VAL[Validators / Jito]
    end

    subgraph Bot["Arbitrage Bot"]
        EXE[Executor]
        RISK[Risk Manager]
        JITO[Jito Client]
        PATH[Pathfinder]
    end

    subgraph Config["Configuration"]
        ENV[.env]
    end

    J --> EXE
    ENV --> EXE
    
    EXE --> RISK
    RISK --> PATH
    
    path --> EXE
    
    EXE -- "Transaction Bundle" --> JITO
    EXE -- "Direct Tx" --> VAL
    JITO -- "MEV Bundle" --> VAL
```

## 🧪 Simulation Data

The bot includes a robust simulation mode that logs potential trades without executing them.

| Pair | Loop | Profit % | Est. Profit |
|------|------|----------|-------------|
| **BONK/SOL** | SOL->USDC->BONK->SOL | **2.40%** | $13.27 |
| **RAY/USDC** | USDC->RAY->SOL->USDC | **2.28%** | $15.53 |

## 📁 Project Structure

```
solana-arbitrage/
├── crates/             
│   ├── bot/            # MAIN TRADING BINARY (Production)
│   ├── core/           # Shared logic, pricing, pathfinding
│   ├── collector/      # (Maintenance Mode)
│   └── api/            # (Maintenance Mode)
├── docs/               # Architecture docs & logs
└── .env.example        # Configuration template
```

## License

MIT
