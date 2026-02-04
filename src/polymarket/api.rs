use anyhow::Result;
use async_trait::async_trait;
use super::types::MarketData;
use polymarket_client_sdk::clob::types::OrderType;

#[async_trait]
pub trait MarketInterface: Send + Sync {
    /// Fetch all active markets
    async fn get_active_markets(&self) -> Result<Vec<MarketData>>;
    
    /// Get details for a specific market
    async fn get_market_details(&self, market_id: &str) -> Result<MarketData>;
    
    /// Get account balance (USDC)
    async fn get_balance(&self) -> Result<f64>;
    
    /// Place an order
    async fn place_order(
        &self,
        market_id: &str,
        side: &str,
        size: f64,
        price: f64,
        order_type: OrderType,
    ) -> Result<String>;
}
