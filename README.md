# Polymarket Sniper Bot

A High-Frequency Trading (HFT) Sniper Bot designed for deterministic speed and precision on Polymarket. This bot bypasses traditional UI interfaces and interacts directly with the CLOB (Central Limit Order Book) to capture arbitrage opportunities in nanoseconds.

## Features

*   **⚡ Zero Latency Architecture**: Direct WebSocket integration with Polymarket CLOB.
*   **🎯 Intra-Market Arbitrage**: Detecting price inefficiencies (e.g., Yes + No < 1.0) and executing instantly.
*   **🔌 Real-Time L2 Orderbook**: Processing Bid/Ask updates in real-time.
*   **🛡️ Risk Management**: Strict position sizing and PnL-based stop-losses.
*   **🔎 Mempool Monitoring**: (Optional) Monitoring pending transactions via Polygon RPC.
*   **🧪 Paper Trading**: Safe simulation mode included.

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
*   `POLYMARKET_API_KEY`: Your Polymarket API Key (Proxy Key).
*   `POLYMARKET_SECRET`: Your Polymarket API Secret.
*   `POLYMARKET_PASSPHRASE`: Your Polymarket API Passphrase.

### 3. Execution

Run the sniper in release mode:

```bash
cargo run --release
```

## Architecture

The system follows a deterministic pipeline:

```
Market Stream (WS) -> Strategy (Rule Engine) -> Risk Check -> Executor
```

1.  **Market Monitor**: Subscribes to `Level2` orderbook updates via WebSocket.
2.  **Sniper Strategy**: 
    - Checks `Yes + No` price sums.
    - If `Sum < 1.0 - Fees - MinEdge`, triggers a `BuyBoth` signal.
3.  **Risk Manager**: Validates position limits and portfolio exposure.
4.  **Executor**: Submits signed orders instantly to the Exchange.

## Performance Benchmarks

*   **Logic Latency**: ~17 nanoseconds (Verified via `cargo bench`).
*   **Network Latency**: Depends on location relative to Polymarket CLOB servers.

## Development

*   `src/main.rs`: Entry point.
*   `src/sniper.rs`: Core engine loop.
*   `src/strategies/arbitrage.rs`: Trading logic.
*   `src/polymarket/ws.rs`: WebSocket client implementation.

## License

MIT License

## Disclaimer

**Trading cryptocurrency and prediction markets involves significant risk.** This software is provided for educational and experimental purposes only. The authors assume no responsibility for financial losses incurred while using this software. Always trade responsibly.
