# Polymarket Sniper Bot

A High-Frequency Trading (HFT) Sniper Bot designed for deterministic speed and precision on Polymarket. This bot bypasses traditional UI interfaces and interacts directly with the CLOB (Central Limit Order Book) to capture arbitrage opportunities in **nanoseconds**.

## ⚡ Performance

**Decision Latency**: **16.7ns** (59x faster than 1μs HFT requirement)

| Component | Latency | Status |
|-----------|---------|--------|
| Decision Pipeline | 16.7ns | ✅ Excellent |
| Orderbook Helpers | 0.3ns | ✅ Excellent |
| Kelly Calculation | 2.0ns | ✅ Excellent |
| Total Liquidity | 3.3ns | ✅ Excellent |

**Optimizations Applied**:
- Fixed-size arrays (zero-allocation)
- CPU pinning (core isolation)
- Inline hints on hot paths
- Lock-free data structures
- Memory arena allocator

## Features

*   **⚡ Ultra-Low Latency**: 16.7ns decision making with zero allocations
*   **🎯 Intra-Market Arbitrage**: Detecting price inefficiencies (Yes + No < 1.0)
*   **🔌 Real-Time L2 Orderbook**: Full depth analysis with 50-level orderbook
*   **💰 Dynamic Position Sizing**: Kelly Criterion with volatility adjustment
*   **🛡️ MEV Protection**: Flashbots integration for private transactions
*   **🔎 Mempool Monitoring**: Copy-trading detection and front-running prevention
*   **🧪 Paper Trading**: Safe simulation mode included
*   **📊 Live Dashboard**: Real-time PnL and performance metrics

## Quick Start

### 1. Installation

Build the project in release mode for maximum optimization:

```bash
cd polymarket-hft-agent
cargo build --release
```

### 2. Configuration

Create a configuration file from the example template:

```bash
cp .env.example .env
nano .env
```

**Required Credentials:**
*   `POLYMARKET_API_KEY`: Your Polymarket API Key (Proxy Key)
*   `POLYMARKET_SECRET`: Your Polymarket API Secret
*   `POLYMARKET_PASSPHRASE`: Your Polymarket API Passphrase
*   `POLYGON_WS_RPC`: Polygon WebSocket RPC endpoint
*   `POLYGON_PRIVATE_KEY`: Your wallet private key

**Optional (Flashbots)**:
*   `FLASHBOTS_ENABLED=true`: Enable MEV protection
*   `FLASHBOTS_RELAY_URL`: Flashbots relay endpoint

### 3. Execution

Run the sniper in release mode:

```bash
cargo run --release
```

Access dashboard at `http://localhost:3002`

## Architecture

```
WebSocket (Core 1) → Lock-Free Queue → Strategy (Core 0) → Flashbots → CLOB
                                              ↓
                                       Memory Arena
```

1.  **Market Monitor**: Subscribes to `Level2` orderbook updates via WebSocket
2.  **Sniper Strategy**: 
    - Analyzes full orderbook depth (50 levels)
    - Calculates weighted average prices and slippage
    - Dynamic position sizing using Kelly Criterion
    - If `Sum < 1.0 - Fees - MinEdge`, triggers `BuyBoth` signal
3.  **Risk Manager**: Validates position limits and portfolio exposure
4.  **Executor**: Submits atomic bundles via Flashbots or regular transactions

## Performance Benchmarks

Verified via `cargo bench --bench latency`:

```
Decision Pipeline:        16.7ns  (-8.2% vs baseline)
├─ Opportunity Check:     17.7ns
├─ Orderbook Analysis:    4-14ns (depth-dependent)
├─ Kelly Calculation:     2.0ns
└─ Position Sizing:       <1ns

Orderbook Helpers:
├─ best_bid/ask:          0.29ns (-12%)
├─ total_ask_liquidity:   3.3ns  (-41%)
└─ total_bid_liquidity:   6.0ns
```

**Network Latency**: Depends on location relative to Polymarket CLOB servers

## Development

### Project Structure

*   `src/main.rs`: Entry point with CPU pinning
*   `src/sniper.rs`: Core engine loop
*   `src/strategies/arbitrage.rs`: Trading logic with L2 analysis
*   `src/strategies/position_sizing.rs`: Kelly Criterion implementation
*   `src/polymarket/ws.rs`: WebSocket client
*   `src/polymarket/lockfree_queue.rs`: Lock-free SPSC queue
*   `src/execution/flashbots.rs`: MEV protection
*   `src/execution/cpu_affinity.rs`: CPU core pinning

### Running Benchmarks

```bash
cargo bench --bench latency
```

### Running Tests

```bash
cargo test
```

## License

MIT License

## Disclaimer

**Trading cryptocurrency and prediction markets involves significant risk.** This software is provided for educational and experimental purposes only. The authors assume no responsibility for financial losses incurred while using this software. Always trade responsibly.
