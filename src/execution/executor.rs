use anyhow::Result;
use tracing::info;

use crate::strategies::risk::RiskManager;
use crate::strategies::types::TradingDecision;
use crate::polymarket::{MarketData, PolymarketClient, MarketInterface};

pub struct Executor {
    market_interface: Box<dyn MarketInterface>,
}

impl Executor {
    pub fn new(market_interface: Box<dyn MarketInterface>) -> Self {
        Self {
            market_interface,
        }
    }

    /// Execute a trading decision
    pub async fn execute_trade(
        &self,
        decision: &TradingDecision,
        market: &MarketData,
        trade_id: &str,
        risk_manager: &mut RiskManager,
    ) -> Result<String> {
        if !decision.should_trade {
            info!("⏭️  Skipping trade for market: {}", market.question);
            return Ok("".to_string());
        }

        info!("🚀 Executing trade for market: {}", market.question);

        // Determine price (TODO: Use order book for better accuracy)
        let price = if decision.side == "YES" {
            market.yes_price
        } else {
            market.no_price
        };

        // Calculate actual position size in dollars
        let capital = self.market_interface.get_balance().await.unwrap_or(1000.0);
        let position_size_usd = capital * decision.position_size_pct;

        // Place order
        let order_id = self
            .market_interface
            .place_order(
                &market.id,
                &decision.side,
                position_size_usd,
                price,
            )
            .await?;

        info!("✅ Order placed! ID: {}", order_id);

        // Track position in Risk Manager
        risk_manager.add_position(
            market.id.clone(),
            trade_id.to_string(),
            decision.side.clone(),
            position_size_usd,
            price,
        );

        Ok(order_id)
    }

    /// Close a position
    pub async fn close_position(
        &self,
        market: &MarketData,
        side: &str,
        risk_manager: &mut RiskManager,
    ) -> Result<()> {
        info!("📉 Closing position for market: {}", market.question);
        
        let price = if side == "YES" {
            market.yes_price
        } else {
            market.no_price
        };

        // Determine opposite side to close
        let close_side = if side == "YES" { "NO" } else { "YES" };

        let order_id = self
            .market_interface
            .place_order(
                &market.id,
                close_side,
                0.0, // Size 0.0 implies "Close All" in our hypothetical logic or we need to track size
                price,
            )
            .await?;

        info!("✅ Close order placed: {}", order_id);
        
        risk_manager.remove_position(&market.id);
        
        Ok(())
    }
}
