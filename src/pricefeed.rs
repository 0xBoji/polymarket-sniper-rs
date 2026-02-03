use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::{Duration, Instant};
const BINANCE_API_URL: &str = "https://api.binance.com/api/v3/ticker/price";
const CACHE_DURATION: Duration = Duration::from_millis(500);

#[derive(Debug, Deserialize)]
struct BinancePriceResponse {
    #[allow(dead_code)]
    symbol: String,
    price: String,
}

#[derive(Clone)]
struct CachedPrice {
    price: f64,
    timestamp: Instant,
}

pub struct BinanceClient {
    http_client: Client,
    cache: Arc<RwLock<HashMap<String, CachedPrice>>>,
}

impl BinanceClient {
    pub fn new() -> Self {
        Self {
            http_client: Client::builder()
                .timeout(Duration::from_secs(2))
                .build()
                .unwrap_or_default(),
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get_price(&self, symbol: &str) -> Result<f64> {
        let symbol = symbol.to_uppercase();

        // 1. Check cache
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(&symbol) {
                if cached.timestamp.elapsed() < CACHE_DURATION {
                    return Ok(cached.price);
                }
            }
        }

        // 2. Fetch from Binance
        let url = format!("{}?symbol={}", BINANCE_API_URL, symbol);
        let resp = self.http_client.get(url).send().await?;

        if !resp.status().is_success() {
            anyhow::bail!("Binance API returned status {}", resp.status());
        }

        let data: BinancePriceResponse = resp.json().await?;
        let price: f64 = data.price.parse()?;

        // 3. Update cache
        {
            let mut cache = self.cache.write().await;
            cache.insert(symbol, CachedPrice {
                price,
                timestamp: Instant::now(),
            });
        }

        Ok(price)
    }

    pub fn symbol_from_question(question: &str) -> Option<&'static str> {
        let q = question.to_lowercase();
        if q.contains("bitcoin") || q.contains("btc") {
            Some("BTCUSDT")
        } else if q.contains("ethereum") || q.contains("eth") {
            Some("ETHUSDT")
        } else if q.contains("solana") || q.contains("sol") {
            Some("SOLUSDT")
        } else if q.contains("xrp") || q.contains("ripple") {
            Some("XRPUSDT")
        } else {
            None
        }
    }
}
