use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use anyhow::Result;
use tracing::{info, error, warn, debug};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

const CLOB_WS_URL: &str = "wss://ws-subscriptions-clob.polymarket.com/ws/market";

#[derive(Debug, Clone, Serialize)]
pub struct Subscription {
    pub assets_ids: Vec<String>,
    #[serde(rename = "type")]
    pub msg_type: String,
}





// NEW STRUCTS DEFINITION
#[derive(Debug, Deserialize)]
pub struct WsMessage {
    pub market: String,
    pub price_changes: Vec<WsPriceChange>,
    pub timestamp: String,
    pub event_type: String,
}

#[derive(Debug, Deserialize)]
pub struct WsPriceChange {
    pub asset_id: String,
    pub price: String,
    pub size: String,
    pub side: String,
    pub hash: String,
    pub best_bid: String,
    pub best_ask: String,
}

#[derive(Debug, Deserialize)]
pub struct OrderbookUpdate {
    pub asset_id: String,
    pub bids: Vec<PriceLevel>, // Kept for compatibility with sniper.rs
    pub asks: Vec<PriceLevel>,
    pub timestamp: String,
    pub hash: String,
}

#[derive(Debug, Deserialize)]
pub struct PriceLevel {
    pub price: String,
    pub size: String,
}

pub struct ClobWebSocket {
    // We might need to send subscriptions dynamically
    subscribe_tx: mpsc::UnboundedSender<Vec<String>>,
}

impl ClobWebSocket {
    pub async fn new(
        update_tx: mpsc::Sender<OrderbookUpdate>
    ) -> Result<Self> {
        let (subscribe_tx, mut subscribe_rx) = mpsc::unbounded_channel::<Vec<String>>();

        tokio::spawn(async move {
            loop {
                info!("🔌 Connecting to CLOB WebSocket: {}", CLOB_WS_URL);
                match connect_async(CLOB_WS_URL).await {
                    Ok((ws_stream, _)) => {
                        info!("✅ CLOB WebSocket Connected!");
                        let (mut write, mut read) = ws_stream.split();

                        // Keep registration of new subs
                        // In a real robust implem, we should re-subscribe to everything on reconnect.
                        // For now, we listen for new requests.
                        
                        let mut pending_subs: Vec<String> = Vec::new();
                        let mut flush_interval = tokio::time::interval(std::time::Duration::from_millis(200));

                        loop {
                            tokio::select! {
                                Some(msg) = read.next() => {
                                    match msg {
                                        Ok(Message::Text(text)) => {
                                            // FORCE LOG for debugging
                                            if text.contains("asset_id") || text.contains("error") {
                                                info!("📩 WS Rx: {}", text); 
                                            }
                                            
                                            // Correctly parse as Object (WsMessage) not Array
                                            match serde_json::from_str::<WsMessage>(&text) {
                                                Ok(msg) => {
                                                    // Convert WsMessage to Vec<OrderbookUpdate> for compatibility
                                                    // or just send individual updates.
                                                    for change in msg.price_changes {
                                                        let update = OrderbookUpdate {
                                                            asset_id: change.asset_id,
                                                            bids: vec![PriceLevel { price: change.best_bid, size: "0".to_string() }], // Dummy Vec for compat
                                                            asks: vec![PriceLevel { price: change.best_ask, size: "0".to_string() }], // Dummy Vec for compat
                                                            timestamp: msg.timestamp.clone(),
                                                            hash: change.hash,
                                                        };
                                                        
                                                        if let Err(e) = update_tx.send(update).await {
                                                            error!("❌ Failed to send update to agent: {}", e);
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    // Only warn if it looks like a market update (contains "asset_id")
                                                    if text.contains("asset_id") {
                                                        error!("❌ Failed to parse WS update: {}. Text: {}", e, text);
                                                    } else if !text.contains("check_ka") { 
                                                        debug!("ℹ️ Ignored system msg: {}", text);
                                                    }
                                                }
                                            }
                                        }

// ... (Ping/Close handlers remain same) ...


                                        Ok(Message::Ping(ping)) => {
                                             if let Err(_) = write.send(Message::Pong(ping)).await {
                                                 break;
                                             }
                                        }
                                        Ok(Message::Close(_)) => {
                                            warn!("🔌 CLOB WebSocket closed");
                                            break;
                                        }
                                        Err(e) => {
                                            error!("❌ CLOB WebSocket Error: {}", e);
                                            break;
                                        }
                                        _ => {}
                                    }
                                }
                                Some(mut assets) = subscribe_rx.recv() => {
                                    pending_subs.append(&mut assets);
                                }
                                _ = flush_interval.tick() => {
                                    if !pending_subs.is_empty() {
                                        let batch: Vec<String> = pending_subs.drain(..).collect();
                                        // Chunking to avoid too large frames (e.g. 50 assets per msg)
                                        for chunk in batch.chunks(50) {
                                            let sub = Subscription {
                                                assets_ids: chunk.to_vec(),
                                                msg_type: "market".to_string(),
                                            };
                                            let json = serde_json::to_string(&sub).unwrap_or_default();
                                            info!("📤 Sending Sub: {}", json);
                                            if let Err(e) = write.send(Message::Text(json)).await {
                                                error!("❌ Failed to send batch subscription: {}", e);
                                            }
                                        }
                                    }
                                }
                                else => break,
                            }
                        }
                    }
                    Err(e) => {
                        error!("❌ Connection failed: {}", e);
                    }
                }
                
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                info!("🔄 Reconnecting CLOB WebSocket...");
            }
        });

        Ok(Self { subscribe_tx })
    }

    pub fn subscribe(&self, asset_ids: Vec<String>) {
        if let Err(e) = self.subscribe_tx.send(asset_ids) {
            error!("❌ Failed to queue subscription: {}", e);
        }
    }
}


