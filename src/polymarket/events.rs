use ethers::prelude::*;
use std::sync::Arc;
use tokio::sync::mpsc;
use anyhow::Result;
use tracing::{info, error, warn};

/// Listens to Polygon blockchain events for new market creation
pub struct MarketEventListener {
    provider: Arc<Provider<Ws>>,
    ctf_address: Address,
}

impl MarketEventListener {
    /// Create a new event listener
    pub async fn new(ws_url: &str, ctf_address: Address) -> Result<Self> {
        info!("🔌 Connecting to Polygon WebSocket: {}", ws_url);
        let provider = Provider::<Ws>::connect(ws_url).await?;
        info!("✅ WebSocket connected successfully");
        
        Ok(Self {
            provider: Arc::new(provider),
            ctf_address,
        })
    }

    /// Listen for new market creation events
    /// Sends condition IDs through the channel when new markets are created
    pub async fn listen_for_new_markets(
        &self,
        tx: mpsc::UnboundedSender<String>,
    ) -> Result<()> {
        info!("👂 Starting to listen for new market events...");
        
        // Create filter for ConditionPreparation events
        // Event signature: ConditionPreparation(bytes32 indexed conditionId, address indexed oracle, bytes32 indexed questionId, uint256 outcomeSlotCount)
        let filter = Filter::new()
            .address(self.ctf_address)
            .event("ConditionPreparation(bytes32,address,bytes32,uint256)");

        match self.provider.subscribe_logs(&filter).await {
            Ok(mut stream) => {
                info!("⚡ WebSocket event stream started - listening for new markets");
                
                while let Some(log) = stream.next().await {
                    // Extract condition ID from event (first indexed parameter)
                    if log.topics.len() > 1 {
                        let condition_id = format!("{:?}", log.topics[1]);
                        
                        info!("🆕 NEW MARKET EVENT: Condition ID {}", condition_id);
                        
                        // Send to processing queue
                        if let Err(e) = tx.send(condition_id.clone()) {
                            error!("Failed to send condition ID to queue: {}", e);
                        }
                    }
                }
                
                warn!("⚠️ WebSocket stream ended unexpectedly");
            }
            Err(e) => {
                error!("❌ Failed to subscribe to logs: {}", e);
                return Err(e.into());
            }
        }

        Ok(())
    }
}
