use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub polymarket: PolymarketConfig,
    pub arbitrage: ArbitrageConfig,
    pub agent: AgentConfig,
    pub risk: RiskConfig,
    pub market_filters: MarketFilters,
    pub flashbots: FlashbotsConfig,
    pub polygon_ws_rpc: Option<String>,
    pub polygon_private_key: Option<String>,
    pub ctf_contract_address: Option<String>,
    pub expiration: ExpirationConfig,
    pub predictive: PredictiveConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PolymarketConfig {
    pub api_key: String,
    pub secret: String,
    pub passphrase: String,
    pub host: String,
    pub proxy_address: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ArbitrageConfig {
    pub min_edge_bps: i32,
    pub max_position_size_usd: f64,
    // Dynamic position sizing
    pub use_dynamic_sizing: bool,
    pub kelly_fraction: f64,
    pub min_position_pct: f64,
    pub max_position_pct: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExpirationConfig {
    pub enabled: bool,
    pub max_time_remaining_sec: u64,
    pub min_price: f64,
    pub target_price: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PredictiveConfig {
    pub enabled: bool,
    pub min_confidence: f64,
    pub max_uncertainty: f64,
    pub binance_signal_threshold_pct: f64,
}


#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentConfig {
    pub paper_trading: bool,
    pub simulation_mode: bool,
    pub market_poll_interval_secs: u64,
    pub scan_existing_on_startup: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RiskConfig {
    pub max_position_size_pct: f64,
    pub max_portfolio_exposure_pct: f64,
    pub stop_loss_pct: f64,
    pub use_dynamic_sl: bool,
    pub min_hold_time_secs: u64,
    pub auto_sell_threshold: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MarketFilters {
    pub min_market_volume: f64,
    pub min_liquidity: f64,
    pub min_24h_volume: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FlashbotsConfig {
    pub enabled: bool,
    pub relay_url: String,
    pub signing_key: Option<String>,
    pub max_retries: u32,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();

        let polymarket = PolymarketConfig {
            // SDK auto-generates credentials from private key - these are optional
            api_key: env::var("POLYMARKET_API_KEY").unwrap_or_default(),
            secret: env::var("POLYMARKET_SECRET").unwrap_or_default(),
            passphrase: env::var("POLYMARKET_PASSPHRASE").unwrap_or_default(),
            host: env::var("POLYMARKET_HOST")
                .unwrap_or_else(|_| "https://clob.polymarket.com".to_string()),
            proxy_address: env::var("POLYMARKET_PROXY_ADDRESS").ok(),
        };

        let arbitrage = ArbitrageConfig {
            min_edge_bps: env::var("MIN_EDGE_BPS")
                .unwrap_or_else(|_| "20".to_string())
                .parse()
                .unwrap_or(20),
            max_position_size_usd: env::var("MAX_POSITION_SIZE_USD")
                .unwrap_or_else(|_| "10.0".to_string())
                .parse()
                .unwrap_or(10.0),
            use_dynamic_sizing: env::var("USE_DYNAMIC_SIZING")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            kelly_fraction: env::var("KELLY_FRACTION")
                .unwrap_or_else(|_| "0.25".to_string())
                .parse()
                .unwrap_or(0.25),
            min_position_pct: env::var("MIN_POSITION_PCT")
                .unwrap_or_else(|_| "0.01".to_string())
                .parse()
                .unwrap_or(0.01),
            max_position_pct: env::var("MAX_POSITION_PCT")
                .unwrap_or_else(|_| "0.10".to_string())
                .parse()
                .unwrap_or(0.10),
        };

        let expiration = ExpirationConfig {
            enabled: env::var("EXPIRATION_SNIPING_ENABLED")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            max_time_remaining_sec: env::var("EXPIRATION_MAX_TIME_SEC")
                .unwrap_or_else(|_| "60".to_string())
                .parse()
                .unwrap_or(60),
            min_price: env::var("EXPIRATION_MIN_PRICE")
                .unwrap_or_else(|_| "0.92".to_string())
                .parse()
                .unwrap_or(0.92),
            target_price: env::var("EXPIRATION_TARGET_PRICE")
                .unwrap_or_else(|_| "0.99".to_string())
                .parse()
                .unwrap_or(0.99),
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
            scan_existing_on_startup: env::var("SCAN_EXISTING_ON_STARTUP")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
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
            use_dynamic_sl: env::var("USE_DYNAMIC_SL")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            min_hold_time_secs: env::var("MIN_HOLD_TIME_SECS")
                .unwrap_or_else(|_| "60".to_string())
                .parse()
                .unwrap_or(60),
            auto_sell_threshold: env::var("AUTO_SELL_THRESHOLD")
                .unwrap_or_else(|_| "0.99".to_string())
                .parse()
                .unwrap_or(0.99),
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
            min_24h_volume: env::var("MIN_24H_VOLUME")
                .unwrap_or_else(|_| "0.0".to_string())
                .parse()
                .unwrap_or(0.0),
        };

        let polygon_ws_rpc = env::var("POLYGON_WS_RPC").ok();
        let polygon_private_key = env::var("POLYGON_PRIVATE_KEY").ok();
        let ctf_contract_address = env::var("CTF_CONTRACT_ADDRESS").ok();

        let flashbots = FlashbotsConfig {
            enabled: env::var("USE_FLASHBOTS")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            relay_url: env::var("FLASHBOTS_RELAY_URL")
                .unwrap_or_else(|_| "https://relay.flashbots.net".to_string()),
            signing_key: env::var("FLASHBOTS_SIGNING_KEY").ok(),
            max_retries: env::var("MAX_BUNDLE_RETRIES")
                .unwrap_or_else(|_| "3".to_string())
                .parse()
                .unwrap_or(3),
        };

        Ok(Config {
            polymarket,
            arbitrage,
            agent,
            risk,
            market_filters,
            flashbots,
            polygon_ws_rpc,
            polygon_private_key,
            ctf_contract_address,
            expiration,
            predictive: PredictiveConfig {
                enabled: env::var("PREDICTIVE_SNIPING_ENABLED")
                    .unwrap_or_else(|_| "false".to_string())
                    .parse()
                    .unwrap_or(false),
                min_confidence: env::var("PREDICTIVE_MIN_CONFIDENCE")
                    .unwrap_or_else(|_| "0.50".to_string())
                    .parse()
                    .unwrap_or(0.50),
                max_uncertainty: env::var("PREDICTIVE_MAX_UNCERTAINTY")
                    .unwrap_or_else(|_| "0.10".to_string())
                    .parse()
                    .unwrap_or(0.10),
                binance_signal_threshold_pct: env::var("BINANCE_SIGNAL_THRESHOLD_PCT")
                    .unwrap_or_else(|_| "0.5".to_string())
                    .parse()
                    .unwrap_or(0.5),
            },
        })
    }
}
