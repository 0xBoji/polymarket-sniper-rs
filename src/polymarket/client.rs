use anyhow::Result;
use polyfill_rs::{ClobClient, Market, OrderArgs};
use polyfill_rs::types::{Side, ApiCredentials as ApiCreds, OrderType, BalanceAllowanceParams, AssetType};
use rust_decimal::Decimal;
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
        // FIX: The correct query parameter is `condition_ids` (plural), not `condition_id`
        let url = format!("{}/markets?condition_ids={}", self.gamma_url, market_id);
        
        let response = self.http_client
            .get(&url)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            warn!("Gamma API error for {}: {} - {}", market_id, status, error_text);
            anyhow::bail!("Gamma API error: {} - {}", status, error_text);
        }

        let markets: Vec<GammaMarket> = response.json().await?;
        
        // Check Gamma findings
        let market_gamma = markets
            .into_iter()
            .find(|m| m.condition_id == market_id);

        if let Some(m) = market_gamma {
            return self.convert_gamma_market(&m);
        }
        
        // FALLBACK: If not found in Gamma, try local derivation
        info!("⚠️ Market {} not in Gamma yet. Attempting local derivation.", market_id);
        
        match crate::polymarket::contracts::derive_asset_ids(market_id) {
            Ok((yes_id, no_id)) => {
                 info!("✅ Derived IDs for {}: YES={}, NO={}", market_id, yes_id, no_id);
                 // Assuming binary market: NO is index 0, YES is index 1
                 Ok(MarketData {
                    id: market_id.to_string(),
                    question: format!("Market {}", market_id), // Placeholder
                    end_date: None,
                    volume: 0.0,
                    liquidity: 0.0,
                    yes_price: 0.0, 
                    no_price: 0.0,
                    volume_24h: 0.0,
                    description: None,
                    order_book_imbalance: 0.0,
                    best_bid: 0.0,
                    best_ask: 0.0,
                    asset_ids: vec![no_id, yes_id], 
                 })
            },
            Err(e) => {
                 warn!("❌ Failed to derive IDs: {}", e);
                 anyhow::bail!("Market not found in Gamma and derivation failed: {}", market_id)
            }
        }
    }

    async fn get_balance(&self) -> Result<f64> {
        if self.paper_trading {
            return Ok(1000.0); // Dummy balance for paper trading
        }

        let params = BalanceAllowanceParams {
            asset_type: Some(AssetType::COLLATERAL),
            ..Default::default()
        };

        let response = self.client.get_balance_allowance(Some(params)).await?;
        
        // Response is usually a map of token_id -> { balance, allowance }
        // For collateral, we expect the collateral token (USDC).
        // Since we filtered by COLLATERAL, we can iterate and find the first positive balance 
        // or just the known collateral address if we had it.
        
        // Example response: {"0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174": {"balance": "100.0", "allowance": "..."}}
        
        if let Some(map) = response.as_object() {
            for (_token_id, data) in map {
                if let Some(balance_str) = data.get("balance").and_then(|v| v.as_str()) {
                    let balance: f64 = balance_str.parse().unwrap_or(0.0);
                    return Ok(balance);
                }
            }
        }
        
        Ok(0.0)
    }

    async fn place_order(
        &self,
        market_id: &str,
        side_str: &str,
        size_usd: f64,
        price_f64: f64,
    ) -> Result<String> {
        if self.paper_trading {
            info!(
                "📝 [PAPER] Order: {} ${:.2} @ ${:.4} on market {}",
                side_str, size_usd, price_f64, market_id
            );
            return Ok(format!("paper-order-{}", uuid::Uuid::new_v4()));
        }

        info!(
            "🚨 LIVE ORDER: {} ${:.2} @ ${:.4} on market {}",
            side_str, size_usd, price_f64, market_id
        );

        // 1. Resolve Token ID and Side
        let side = match side_str.to_uppercase().as_str() {
            "BUY" | "YES" => Side::BUY,
            "SELL" | "NO" => Side::SELL,
            _ => anyhow::bail!("Invalid side: {}", side_str),
        };

        // We need the token_id. If we don't have it, we must fetch market details.
        // For sniper, we usually have it in MarketData if it was processed.
        // But the interface only gives market_id (condition_id).
        let market_details = self.get_market_details(market_id).await?;
        
        // Polymarket CTF: YES is usually index 1, NO is index 0.
        // Our MarketData.asset_ids order depends on how it was fetched/derived.
        // In sniper.rs, for GOD MODE: asset_ids[0]=NO, asset_ids[1]=YES.
        // In client.rs convert_gamma_market: it uses clob_token_ids order from Gamma.
        
        let token_id = if side_str.to_uppercase() == "YES" || side_str.to_uppercase() == "BUY" {
            // Find YES token. Historically index 1.
            if market_details.asset_ids.len() >= 2 {
                market_details.asset_ids[1].clone()
            } else {
                anyhow::bail!("Market {} missing YES token ID", market_id);
            }
        } else {
            // Find NO token. Historically index 0.
            if !market_details.asset_ids.is_empty() {
                market_details.asset_ids[0].clone()
            } else {
                anyhow::bail!("Market {} missing NO token ID", market_id);
            }
        };

        // 2. Convert to Decimal
        let price = Decimal::from_f64_retain(price_f64).ok_or_else(|| anyhow::anyhow!("Invalid price"))?;
        
        // Polymarket orders are in units of tokens. 
        // 1 token = $price USD.
        // size_usd = token_size * price
        // token_size = size_usd / price
        let token_size_f64 = size_usd / price_f64;
        let size = Decimal::from_f64_retain(token_size_f64).ok_or_else(|| anyhow::anyhow!("Invalid size"))?;

        // 3. Create Order
        let order_args = OrderArgs::new(&token_id, price, size, side);
        
        // Sign the order (EIP-712)
        // ClobClient::create_order handles the signing if initialized with a private key
        let signed_order = self.client.create_order(
            &order_args,
            None, // expiration (default 0)
            None, // extras
            None, // options
        ).await.map_err(|e| anyhow::anyhow!("Failed to sign order: {}", e))?;

        // 4. Post Order
        let response = self.client.post_order(
            signed_order,
            OrderType::GTC, // Good Til Cancelled
        ).await.map_err(|e| anyhow::anyhow!("Failed to post order: {}", e))?;

        let order_id = response["orderID"].as_str()
            .unwrap_or("unknown")
            .to_string();

        info!("✅ LIVE ORDER SUCCESS: ID {}", order_id);

        Ok(order_id)
    }
}

// Keep inherent impl for helper methods and new
impl PolymarketClient {
    pub fn new(config: &PolymarketConfig, paper_trading: bool, private_key: Option<String>) -> Self {
        let client = if let Some(pk) = private_key {
            info!("🔑 Initializing Authenticated Polymarket Client (Live Mode)");
            let api_creds = ApiCreds {
                api_key: config.api_key.clone(),
                secret: config.secret.clone(),
                passphrase: config.passphrase.clone(),
            };
            // 137 = Polygon Mainnet
            ClobClient::with_l2_headers(&config.host, &pk, 137, api_creds)
        } else {
            info!("🌐 Initializing Public Polymarket Client (Paper Mode)");
            ClobClient::new(&config.host)
        };
        
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
            volume_24h: 0.0, // convert_market uses polyfill Market which might not have 24h vol easily available
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
            volume_24h: match &market.volume_24hr {
                serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0),
                serde_json::Value::String(s) => s.parse().unwrap_or(0.0),
                _ => 0.0,
            },
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
    pub volume_24hr: serde_json::Value, // Added for popularity filter
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
