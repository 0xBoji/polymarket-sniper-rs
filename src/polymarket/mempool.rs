use ethers::prelude::*;
use std::sync::Arc;
use std::str::FromStr;
use tracing::{error, info, warn};
use dashmap::DashSet;

// use super::contracts::CtfExchangeCalls;

// Polymarket CTF Exchange Address (Polygon)
const CTF_EXCHANGE_ADDRESS: &str = "0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E";

pub struct MempoolMonitor {
    #[allow(dead_code)]
    provider: Option<Arc<Provider<Ws>>>,
    #[allow(dead_code)]
    target_address: Address,
    #[allow(dead_code)]
    watched_addresses: Arc<DashSet<Address>>,
}

impl MempoolMonitor {
    pub async fn new(ws_url: Option<String>) -> Self {
        let provider = if let Some(url) = ws_url {
            match Provider::<Ws>::connect(url).await {
                Ok(p) => {
                    info!("🔌 Connected to WebSocket RPC for Mempool Monitoring");
                    Some(Arc::new(p))
                }
                Err(e) => {
                    warn!("⚠️  Failed to connect to WS RPC: {}", e);
                    None
                }
            }
        } else {
            None
        };

        let target_address = Address::from_str(CTF_EXCHANGE_ADDRESS).unwrap();
        
        let watched_addresses = Arc::new(DashSet::new());

        Self {
            provider,
            target_address,
            watched_addresses,
        }
    }

    #[allow(dead_code)]
    pub fn add_watched_address(&self, address: &str) {
        if let Ok(addr) = Address::from_str(address) {
            self.watched_addresses.insert(addr);
            info!("👀 Monitoring address: {}", address);
        }
    }

    #[allow(dead_code)]
    pub async fn start_monitoring(&self) {
        if let Some(provider) = &self.provider {
            let provider_clone = provider.clone();
            let target = self.target_address;
            let watched = self.watched_addresses.clone();

            tokio::spawn(async move {
                info!("👀 Mempool monitoring started. Listening for pending txs...");
                
                // Subscribe to pending transactions
                let mut stream = match provider_clone.subscribe_pending_txs().await {
                    Ok(s) => s,
                    Err(e) => {
                        error!("❌ Failed to subscribe to pending txs: {}", e);
                        return;
                    }
                };

                while let Some(tx_hash) = stream.next().await {
                    // Fetch full transaction details
                    match provider_clone.get_transaction(tx_hash).await {
                        Ok(Some(tx)) => {
                            // Check destination (Interacting with CTF Exchange?)
                            if let Some(to) = tx.to {
                                if to == target {
                                    handle_exchange_tx(&tx, &watched);
                                }
                            }
                        }
                        Ok(None) => {} 
                        Err(_) => {}   
                    }
                }
            });
        } else {
            warn!("⚠️  Mempool monitoring disabled (no WS URL provided)");
        }
    }
}

#[allow(dead_code)]
fn handle_exchange_tx(tx: &Transaction, watched_addresses: &DashSet<Address>) {
    // 1. Check if 'from' is a watched address
    if watched_addresses.contains(&tx.from) {
        info!("🚨 ALERT: Watchlist address {:?} is interacting with Exchange!", tx.from);
    }

    // 2. Decode Input to see what they are buying/filling
    // We use the generated CtfExchangeCalls enum for decoding
    // TODO: Fix abigen! visibility issue for CtfExchangeCalls
    /*
    if let Ok(decoded) = CtfExchangeCalls::decode(&tx.input) {
        match decoded {
            CtfExchangeCalls::FillOrder(call) => {
                let token_id = call.order.token_id;
                let maker_amount = call.order.maker_amount;
                let outcome_side = if call.order.side == 0 { "BUY" } else { "SELL" }; // 0=BUY, 1=SELL
                
                // Filter out small noise
                if maker_amount > U256::from(100_000_000) { // > 100 USDC (6 decimals)
                     info!(
                        "🌊 Large Order Fill Detected! Side: {} | Token: {} | Amount: {}", 
                        outcome_side, token_id, maker_amount
                    );
                }
            },
            CtfExchangeCalls::Buy(call) => {
                 info!(
                    "🛒 Direct Buy Detected! Condition: {:?} | Outcome: {} | Amount: {}", 
                    call.condition_id, call.outcome_index, call.amount
                );
            },
        }
    }
    */
}
