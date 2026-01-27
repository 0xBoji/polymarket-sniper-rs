use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub polymarket: PolymarketConfig,
    pub arbitrage: ArbitrageConfig,
    pub agent: AgentConfig,
    pub risk: RiskConfig,
    pub market_filters: MarketFilters,
    pub polygon_ws_rpc: Option<String>,
    pub polygon_private_key: Option<String>,
    pub ctf_contract_address: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PolymarketConfig {
    pub api_key: String,
    pub secret: String,
    pub passphrase: String,
    pub host: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ArbitrageConfig {
    pub min_edge_bps: i32,
    pub max_position_size_usd: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentConfig {
    pub paper_trading: bool,
    pub simulation_mode: bool,
    pub market_poll_interval_secs: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RiskConfig {
    pub max_position_size_pct: f64,
    pub max_portfolio_exposure_pct: f64,
    pub stop_loss_pct: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MarketFilters {
    pub min_market_volume: f64,
    pub min_liquidity: f64,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();

        let polymarket = PolymarketConfig {
            api_key: env::var("POLYMARKET_API_KEY")?,
            secret: env::var("POLYMARKET_SECRET")?,
            passphrase: env::var("POLYMARKET_PASSPHRASE")?,
            host: env::var("POLYMARKET_HOST")
                .unwrap_or_else(|_| "https://clob.polymarket.com".to_string()),
        };

        let arbitrage = ArbitrageConfig {
            min_edge_bps: env::var("MIN_EDGE_BPS")
                .unwrap_or_else(|_| "200".to_string())
                .parse()
                .unwrap_or(200),
            max_position_size_usd: env::var("MAX_POSITION_SIZE_USD")
                .unwrap_or_else(|_| "10.0".to_string())
                .parse()
                .unwrap_or(10.0),
        };

        let agent = AgentConfig {
            paper_trading: env::var("PAPER_TRADING")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            simulation_mode: env::var("SIMULATION_MODE")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            market_poll_interval_secs: env::var("MARKET_POLL_INTERVAL_SECS")
                .unwrap_or_else(|_| "5".to_string())
                .parse()
                .unwrap_or(5),
        };

        let risk = RiskConfig {
            max_position_size_pct: env::var("MAX_POSITION_SIZE_PCT")
                .unwrap_or_else(|_| "5.0".to_string())
                .parse()
                .unwrap_or(5.0),
            max_portfolio_exposure_pct: env::var("MAX_PORTFOLIO_EXPOSURE_PCT")
                .unwrap_or_else(|_| "50.0".to_string())
                .parse()
                .unwrap_or(50.0),
            stop_loss_pct: env::var("STOP_LOSS_PCT")
                .unwrap_or_else(|_| "10.0".to_string())
                .parse()
                .unwrap_or(10.0),
        };

        let market_filters = MarketFilters {
            min_market_volume: env::var("MIN_MARKET_VOLUME")
                .unwrap_or_else(|_| "1000.0".to_string())
                .parse()
                .unwrap_or(1000.0),
            min_liquidity: env::var("MIN_LIQUIDITY")
                .unwrap_or_else(|_| "500.0".to_string())
                .parse()
                .unwrap_or(500.0),
        };

        let polygon_ws_rpc = env::var("POLYGON_WS_RPC").ok();
        let polygon_private_key = env::var("POLYGON_PRIVATE_KEY").ok();
        let ctf_contract_address = env::var("CTF_CONTRACT_ADDRESS").ok();

        Ok(Config {
            polymarket,
            arbitrage,
            agent,
            risk,
            market_filters,
            polygon_ws_rpc,
            polygon_private_key,
            ctf_contract_address,
        })
    }
}
