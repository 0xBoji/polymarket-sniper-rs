use anyhow::Result;
use polyfill_rs::{ClobClient, Market};
use serde::Deserialize;
// use std::collections::HashSet;
use tracing::{debug, info, warn};

use crate::config::PolymarketConfig;
use super::types::MarketData;


pub struct PolymarketClient {
    client: ClobClient,
    http_client: reqwest::Client,
    gamma_url: String,
    paper_trading: bool,
}

use super::api::MarketInterface;
use async_trait::async_trait;

#[async_trait]
impl MarketInterface for PolymarketClient {
    // ... (get_active_markets, get_market_details, get_balance same as before)
    async fn get_active_markets(&self) -> Result<Vec<MarketData>> {
        let markets = self.fetch_markets().await?;
        let mut market_data_list = Vec::new();

        for market in markets {
            match self.convert_market(&market) {
                Ok(data) => market_data_list.push(data),
                Err(e) => warn!("Failed to convert market {}: {}", market.question, e),
            }
        }
        
        Ok(market_data_list)
    }

    async fn get_market_details(&self, market_id: &str) -> Result<MarketData> {
        debug!("Fetching details for market (optimized): {}", market_id);
        
        // Optimized: Fetch specific market from Gamma API directly
        // Polymarket "Events" usually map via condition_id
        let url = format!("{}/markets?condition_id={}", self.gamma_url, market_id);
        
        let response = self.http_client
            .get(&url)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Gamma API error: {}", response.status());
        }

        let markets: Vec<GammaMarket> = response.json().await?;
        
        // Check Gamma findings
        let market_gamma = markets
            .into_iter()
            .find(|m| m.condition_id == market_id);

        if let Some(m) = market_gamma {
            return self.convert_gamma_market(&m);
        }
        
        // FALLBACK: If not found in Gamma, try CLOB API (Trading Engine Source)
        warn!("⚠️ Market {} not in Gamma yet, trying CLOB API...", market_id);
        
        let clob_url = format!("https://clob.polymarket.com/markets/{}", market_id);
        let clob_response = self.http_client
            .get(&clob_url)
            .send()
            .await?;

        if clob_response.status().is_success() {
            let market: Market = clob_response.json().await?;
            info!("✅ CLOB Fallback success for {}", market_id);
            return self.convert_market(&market);
        }

        anyhow::bail!("Market not found in Gamma or CLOB: {}", market_id)
    }

    async fn get_balance(&self) -> Result<f64> {
        Ok(1000.0)
    }

    async fn place_order(
        &self,
        market_id: &str,
        side: &str,
        size: f64,
        price: f64,
    ) -> Result<String> {
        if self.paper_trading {
            info!(
                "📝 [PAPER] Order: {} {} @ ${:.4} on market {}",
                side, size, price, market_id
            );
            return Ok(format!("paper-order-{}", uuid::Uuid::new_v4()));
        }

        warn!(
            "🚨 LIVE ORDER: {} {} @ ${:.4} on market {}",
            side, size, price, market_id
        );

        // TODO: Implement actual order placement using polyfill-rs
        anyhow::bail!("Live trading not yet implemented - set PAPER_TRADING=true");
    }
}

// Keep inherent impl for helper methods and new
impl PolymarketClient {
    pub fn new(config: &PolymarketConfig, paper_trading: bool) -> Self {
        let client = ClobClient::new(&config.host);
        
        // Initialize standard HTTP client for Gamma API
        // Initialize optimized HTTP client for HFT
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(4)) // Tighter timeout
            .tcp_nodelay(true) // Disable Nagle's algorithm for lower latency
            .pool_idle_timeout(None) // Keep connections alive indefinitely
            .pool_max_idle_per_host(50) // Allow more concurrent connections
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
            
        let gamma_url = "https://gamma-api.polymarket.com".to_string();
        
        Self {
            client,
            http_client,
            gamma_url,
            paper_trading,
        }
    }

    /// Fetch all active markets
    pub async fn fetch_markets(&self) -> Result<Vec<Market>> {
        debug!("Fetching markets from Polymarket");
        
        let response = self.client.get_sampling_markets(None).await?;
        
        info!("Fetched {} markets", response.data.len());
        Ok(response.data)
    }



    /// Convert polyfill Market to our MarketData
    fn convert_market(&self, market: &Market) -> Result<MarketData> {
        let volume = 0.0; 
        let liquidity = 0.0;

        let mut yes_price = 0.0;
        let mut no_price = 0.0;

        for token in &market.tokens {
            let price_f64 = token.price.to_string().parse::<f64>().unwrap_or(0.0);

            if token.outcome == "Yes" {
                yes_price = price_f64;
            } else if token.outcome == "No" {
                no_price = price_f64;
            }
        }
        
        // Extract asset IDs
        let asset_ids = market.tokens.iter().map(|t| t.token_id.clone()).collect();

        Ok(MarketData {
            id: market.condition_id.clone(),
            question: market.question.clone(),
            end_date: market.end_date_iso.clone(),
            volume,
            liquidity,
            yes_price,
            no_price,
            description: Some(market.description.clone()),
            order_book_imbalance: 0.0,
            best_bid: 0.0,
            best_ask: 0.0,
            asset_ids,
        })
    }


    /// Convert GammaMarket to MarketData
    fn convert_gamma_market(&self, market: &GammaMarket) -> Result<MarketData> {
        let volume: f64 = match &market.volume {
            serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0),
            serde_json::Value::String(s) => s.parse().unwrap_or(0.0),
            _ => 0.0,
        };
        
        let liquidity: f64 = match &market.liquidity {
            serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0),
            serde_json::Value::String(s) => s.parse().unwrap_or(0.0),
            _ => 0.0,
        };

        // Parse outcome prices (stringified JSON array)
        // e.g. "[\"0.5\", \"0.5\"]"
        let outcome_prices: Vec<String> = if market.outcome_prices.is_empty() {
            Vec::new() 
        } else {
            serde_json::from_str(&market.outcome_prices).unwrap_or_default()
        };
        
        let outcomes: Vec<String> = if market.outcomes.is_empty() {
            Vec::new()
        } else {
            serde_json::from_str(&market.outcomes).unwrap_or_default()
        };

        let mut yes_price = 0.0;
        let mut no_price = 0.0;

        // Logic for YES/NO markets
        // Usually index 0 is Long No (or just No), index 1 is Long Yes (or Yes).
        // BUT we should check 'outcomes' strings to be sure if possible.
        // If not standard, default to 0=No, 1=Yes for binary.
        
        if outcome_prices.len() >= 2 {
            // Robust check: try to find "Yes" in outcomes
            let yes_idx = outcomes.iter().position(|s| s.eq_ignore_ascii_case("Yes")).unwrap_or(1);
            let no_idx = outcomes.iter().position(|s| s.eq_ignore_ascii_case("No")).unwrap_or(0);
            
            if yes_idx < outcome_prices.len() {
                yes_price = outcome_prices[yes_idx].parse().unwrap_or(0.0);
            }
            if no_idx < outcome_prices.len() {
                no_price = outcome_prices[no_idx].parse().unwrap_or(0.0);
            }
        }
        
        // Parse clobTokenIds (stringified JSON array)
        let asset_ids: Vec<String> = if market.clob_token_ids.is_empty() {
             Vec::new()
        } else {
             serde_json::from_str(&market.clob_token_ids).unwrap_or_default()
        };

        Ok(MarketData {
            id: market.condition_id.clone(),
            question: market.question.clone(),
            end_date: market.end_date_iso.clone(),
            volume,
            liquidity,
            yes_price,
            no_price,
            description: market.description.clone(),
            order_book_imbalance: 0.0,
            best_bid: 0.0,
            best_ask: 0.0,
            asset_ids,
        })
    }
}

/// Struct matching Gamma API response format
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GammaMarket {
    pub condition_id: String,
    pub question: String,
    // end_date_iso is optional
    pub end_date_iso: Option<String>,
    pub description: Option<String>,
    // outcomes and outcomePrices are JSON strings!
    #[serde(default)]
    pub outcomes: String,       // Default to ""
    #[serde(default)]
    pub outcome_prices: String, // Default to ""
    #[serde(default)]
    pub clob_token_ids: String, // Default to ""
    pub volume: serde_json::Value,     // Can be String or Number
    pub liquidity: serde_json::Value,  // Can be String or Number
}
// Add uuid dependency for order IDs
mod uuid {
    use std::fmt;
    
    pub struct Uuid(String);
    
    impl Uuid {
        pub fn new_v4() -> Self {
            use std::time::{SystemTime, UNIX_EPOCH};
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            Self(format!("{:x}", now))
        }
    }
    
    impl fmt::Display for Uuid {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.0)
        }
    }
}
