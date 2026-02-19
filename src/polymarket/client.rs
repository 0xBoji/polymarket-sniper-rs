use anyhow::Result;
use tracing::{info, warn, debug};
use super::api::MarketInterface;
use async_trait::async_trait;
use alloy::signers::local::PrivateKeySigner;
use alloy::primitives::Address;
use rust_decimal::{Decimal, RoundingStrategy};

    // SDK Imports
use polymarket_client_sdk::{
    POLYGON,
};
use polymarket_client_sdk::clob::Client as ClobClient;
use polymarket_client_sdk::clob::types::{OrderType, Side, SignedOrder, SignableOrder};
use polymarket_client_sdk::clob::types::response::PostOrderResponse;
use polymarket_client_sdk::types::U256;
use polymarket_client_sdk::clob::types::response::MarketResponse;
use polymarket_client_sdk::auth::state::Authenticated;
use polymarket_client_sdk::auth::Normal;
use polymarket_client_sdk::auth::{Signer, LocalSigner};
use polymarket_client_sdk::error::Error as SdkError;
use std::str::FromStr;
use serde::Deserialize; // Only Deserialize is used for GammaMarket

use crate::config::PolymarketConfig;
use crate::polymarket::types::MarketData;

// We need reqwest for Gamma API fallback (http_client)
// But warning said unused `reqwest::Client`.
// Let's check struct definition.
// Struct has `http_client: reqwest::Client`.
// Warning must be because we import it but use full path `reqwest::Client` in struct?
// Line 42: `pub http_client: reqwest::Client`.
// Line 2: `use reqwest::Client;`. 
// Since we use fully qualified `reqwest::Client`, the import is indeed unused. So we remove usage of import.


pub struct PolymarketClient {
    pub client: ClobClient, // Now from new SDK
    pub http_client: reqwest::Client,
    pub gamma_url: String,
    pub paper_trading: bool,
    pub proxy_address: Option<String>,
    // order_builder removed if integrated into ClobClient or handled differently
    // API Credentials for manual requests (Proxy Balance)
    pub api_key: String,
    pub secret: String,
    pub passphrase: String,
    pub signer_address: Address,
    pub private_key: Option<String>,
}

#[async_trait]
impl MarketInterface for PolymarketClient {
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
                 Ok(MarketData {
                    id: market_id.to_string(),
                    question: format!("Market {}", market_id),
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
        // Balance for trading should be checked on proxy/safe wallet if available.
        // Fallback to signer only if proxy is unavailable.
        let target_addr = if let Some(proxy) = &self.proxy_address {
            proxy.clone()
        } else {
            format!("{:?}", self.signer_address)
        };

        if target_addr.is_empty() || target_addr == "0x0000000000000000000000000000000000000000" {
            warn!("⚠️ Unable to determine target wallet for balance check");
            return Ok(0.0);
        }

        // Native USDC: 0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359
        // Bridged USDC: 0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174
        // We check Native first, then Bridged.

        // Selector for balanceOf(address): 70a08231
        // Pad address to 32 bytes (64 hex chars)
        // proxy string is 0x... (42 chars). strip 0x, pad left with 0s to 64 chars.
        let addr_clean = target_addr.trim_start_matches("0x");
        let data = format!("0x70a08231000000000000000000000000{}", addr_clean);

        let mut rpc_candidates = vec![
            "https://polygon-rpc.com".to_string(),
            "https://rpc.ankr.com/polygon".to_string(),
        ];

        if let Ok(ws_rpc) = std::env::var("POLYGON_WS_RPC") {
            let http_rpc = ws_rpc
                .replace("wss://", "https://")
                .replace("ws://", "http://");
            rpc_candidates.insert(0, http_rpc);
        }

        // Dedup and keep order.
        rpc_candidates.dedup();

        for rpc_url in rpc_candidates {
            let native_req = serde_json::json!({
                "jsonrpc": "2.0",
                "method": "eth_call",
                "params": [{
                    "to": "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359",
                    "data": data
                }, "latest"],
                "id": 1
            });

            match self.http_client.post(&rpc_url).json(&native_req).send().await {
                Ok(resp) => {
                    let json: serde_json::Value = match resp.json().await {
                        Ok(v) => v,
                        Err(e) => {
                            warn!("⚠️ Failed to parse balance response from {}: {}", rpc_url, e);
                            continue;
                        }
                    };

                    if let Some(err) = json.get("error") {
                        warn!("⚠️ RPC error from {}: {}", rpc_url, err);
                        continue;
                    }

                    if let Some(result) = json.get("result").and_then(|v| v.as_str()) {
                        if let Ok(amount) = u128::from_str_radix(result.trim_start_matches("0x"), 16) {
                            let balance = amount as f64 / 1_000_000.0;
                            if balance > 0.0 {
                                info!("💰 Native USDC balance on {} via {}: ${:.2}", target_addr, rpc_url, balance);
                                return Ok(balance);
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("⚠️ Balance RPC request failed via {}: {}", rpc_url, e);
                    continue;
                }
            }

            let bridged_req = serde_json::json!({
                "jsonrpc": "2.0",
                "method": "eth_call",
                "params": [{
                    "to": "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174",
                    "data": data
                }, "latest"],
                "id": 2
            });

            match self.http_client.post(&rpc_url).json(&bridged_req).send().await {
                Ok(resp) => {
                    let json: serde_json::Value = match resp.json().await {
                        Ok(v) => v,
                        Err(e) => {
                            warn!("⚠️ Failed to parse bridged balance response from {}: {}", rpc_url, e);
                            continue;
                        }
                    };

                    if let Some(err) = json.get("error") {
                        warn!("⚠️ RPC bridged error from {}: {}", rpc_url, err);
                        continue;
                    }

                    if let Some(result) = json.get("result").and_then(|v| v.as_str()) {
                        if let Ok(amount) = u128::from_str_radix(result.trim_start_matches("0x"), 16) {
                            let balance = amount as f64 / 1_000_000.0;
                            if balance > 0.0 {
                                info!("💰 Bridged USDC balance on {} via {}: ${:.2}", target_addr, rpc_url, balance);
                                return Ok(balance);
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("⚠️ Bridged balance RPC request failed via {}: {}", rpc_url, e);
                    continue;
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
        order_type: OrderType,
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

        let side = match side_str.to_uppercase().as_str() {
            "BUY" | "YES" => Side::Buy,
            "SELL" | "NO" => Side::Sell,
            _ => anyhow::bail!("Invalid side: {}", side_str),
        };

        let market_details = self.get_market_details(market_id).await?;
        
        let token_id = if side_str.to_uppercase() == "YES" || side_str.to_uppercase() == "BUY" {
            if market_details.asset_ids.len() >= 2 {
                market_details.asset_ids[1].clone()
            } else {
                anyhow::bail!("Market {} missing YES token ID", market_id);
            }
        } else {
            if !market_details.asset_ids.is_empty() {
                market_details.asset_ids[0].clone()
            } else {
                anyhow::bail!("Market {} missing NO token ID", market_id);
            }
        };


        // Authenticated Client Setup
        let signer = if let Some(pk) = &self.private_key {
             LocalSigner::from_str(pk)
                 .map_err(|e| anyhow::anyhow!("Invalid private key format: {}", e))?
                 .with_chain_id(Some(POLYGON))
        } else {
             return Err(anyhow::anyhow!("Private key required for signing orders"));
        };

        // Create a NEW unauthenticated client (not clone) to avoid Arc ref count issues
        // SDK's authenticate() consumes the client and requires Arc::into_inner() to succeed
        let fresh_client = ClobClient::new(
            "https://clob.polymarket.com",
            polymarket_client_sdk::clob::Config::default()
        )?;

        // Use SDK-derived Safe wallet as funder
        // This is the signup address shown in Polymarket UI
        let safe_wallet = polymarket_client_sdk::derive_safe_wallet(signer.address(), POLYGON)
            .ok_or_else(|| anyhow::anyhow!("Failed to derive Safe wallet"))?;

        info!("🔐 Using Safe wallet as funder: {}", safe_wallet);

        let auth_client: ClobClient<Authenticated<Normal>> = fresh_client
            .authentication_builder(&signer)
            .signature_type(polymarket_client_sdk::clob::types::SignatureType::GnosisSafe)
            .funder(safe_wallet)
            .authenticate()
            .await?;


        let price = Self::normalize_order_price(price_f64)?;
        let size = Self::normalize_order_size(size_usd, price)?;

        let token_id_u256 = U256::from_str(&token_id).map_err(|e| anyhow::anyhow!("Invalid token ID: {}", e))?;


        // 1. Build Order
        let builder = auth_client.limit_order()
            .token_id(token_id_u256)
            .price(price)
            .size(size)
            .side(side)
            .order_type(order_type);

        let build_res: Result<SignableOrder, SdkError> = builder.build().await;
        let order = build_res.map_err(|e| anyhow::anyhow!("Failed to build order: {}", e))?;

        // 2. Sign Order
        let sign_res: Result<SignedOrder, SdkError> = auth_client.sign(&signer, order).await;
        let signed_order = sign_res.map_err(|e| anyhow::anyhow!("Failed to sign order: {}", e))?;

        // 3. Post Order
        let post_res: Result<PostOrderResponse, SdkError> = auth_client.post_order(signed_order).await;
        let response = post_res.map_err(|e| anyhow::anyhow!("Failed to post order: {}", e))?;

        let order_id = response.order_id;
        info!("✅ LIVE ORDER SUCCESS: ID {}", order_id);

        Ok(order_id)
    }
}


// Keep inherent impl for helper methods and new
impl PolymarketClient {
    fn normalize_order_price(price_f64: f64) -> Result<Decimal> {
        if !price_f64.is_finite() || price_f64 <= 0.0 {
            anyhow::bail!("Invalid price: {}", price_f64);
        }

        // Remove f64 artifacts (e.g. 0.429999999...) and keep price
        // on a practical tick grid for most CLOB markets.
        let parsed = Decimal::from_str(&format!("{:.8}", price_f64))
            .map_err(|e| anyhow::anyhow!("Invalid normalized price: {}", e))?;
        let normalized = parsed.round_dp(2);

        if normalized <= Decimal::ZERO {
            anyhow::bail!("Normalized price is non-positive: {}", normalized);
        }

        Ok(normalized)
    }

    fn normalize_order_size(size_usd: f64, price: Decimal) -> Result<Decimal> {
        if !size_usd.is_finite() || size_usd <= 0.0 {
            anyhow::bail!("Invalid size_usd: {}", size_usd);
        }
        if price <= Decimal::ZERO {
            anyhow::bail!("Invalid normalized price for size conversion: {}", price);
        }

        let size_usd_decimal = Decimal::from_str(&format!("{:.8}", size_usd))
            .map_err(|e| anyhow::anyhow!("Invalid normalized size_usd: {}", e))?;

        let raw_token_size = size_usd_decimal / price;
        let normalized = raw_token_size.round_dp_with_strategy(2, RoundingStrategy::ToZero);

        if normalized <= Decimal::ZERO {
            anyhow::bail!(
                "Normalized token size is zero after lot-size rounding (size_usd=${:.4}, price={})",
                size_usd,
                price
            );
        }

        Ok(normalized)
    }

    pub fn new(config: &PolymarketConfig, paper_trading: bool, private_key: Option<String>) -> Result<Self> {
        
        let http_client = reqwest::Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_secs(4))
            .tcp_nodelay(true)
            .pool_idle_timeout(None)
            .pool_max_idle_per_host(50)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let signer_address = if let Some(pk) = &private_key {
            let signer: PrivateKeySigner = pk.parse().unwrap_or_else(|_| PrivateKeySigner::random());
            signer.address()
        } else {
            Address::ZERO
        };

        // Auto-derive Proxy Address if not in config
        let proxy_address = if config.proxy_address.is_none() && signer_address != Address::ZERO {
            match polymarket_client_sdk::derive_safe_wallet(signer_address, POLYGON) {
                Some(addr) => {
                    info!("🔐 Auto-derived Proxy Wallet: {}", addr);
                    Some(addr.to_string())
                },
                None => {
                    warn!("⚠️ Failed to auto-derive Proxy Wallet");
                    None
                }
            }
        } else {
            config.proxy_address.clone()
        };

        let client = ClobClient::new(&config.host, polymarket_client_sdk::clob::Config::default())?;

        Ok(Self {
            client,
            http_client,
            gamma_url: "https://gamma-api.polymarket.com".to_string(),
            paper_trading,
            proxy_address,
            api_key: config.api_key.clone(),
            secret: config.secret.clone(),
            passphrase: config.passphrase.clone(),
            signer_address,
            private_key,
        })
    }

    /// Fetch all active markets
    pub async fn fetch_markets(&self) -> Result<Vec<MarketResponse>> {
        debug!("Fetching markets from Polymarket");
        
        let response = self.client.sampling_markets(None).await?;
        
        info!("Fetched {} markets", response.data.len());
        Ok(response.data)
    }



    /// Convert polyfill Market to our MarketData
    fn convert_market(&self, market: &MarketResponse) -> Result<MarketData> {
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
        
        // DEBUG: Sampled log to check parsing
        if rand::random::<f64>() < 0.005 { // 0.5% sample
            // info!("🔍 SDK Parse: {} -> YES={:.3} NO={:.3}", market.question, yes_price, no_price);
        }
        
        // Extract asset IDs
        let asset_ids = market.tokens.iter().map(|t| t.token_id.to_string()).collect();

        Ok(MarketData {
            id: market.condition_id.as_ref().map(|b| b.to_string()).unwrap_or_default(),
            question: market.question.clone(),
            end_date: market.end_date_iso.map(|dt| dt.to_string()),
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
    #[serde(default)]
    pub volume_24hr: serde_json::Value, // Added for popularity filter
    pub liquidity: serde_json::Value,  // Can be String or Number
}
