use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;

// Hardcoded known active market (or query one)
// Example: "Will Bitcoin hit $100k in 2024?" (or similar active one)
// Better: Fetch trending market first.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Fetch a market to get token_ids
    println!("🔍 Fetching a market to get Token IDs...");
    let http_client = reqwest::Client::new();
    let markets_url = "https://clob.polymarket.com/sampling-markets";
    
    let resp = http_client.get(markets_url).send().await?.json::<Value>().await?;
    let markets = resp.get("data").and_then(|v| v.as_array()).ok_or_else(|| anyhow::anyhow!("No markets found"))?;
    
    let market = markets.first().ok_or_else(|| anyhow::anyhow!("Empty markets list"))?;
    let question = market.get("question").and_then(|v| v.as_str()).unwrap_or("Unknown");
    let tokens = market.get("tokens").and_then(|v| v.as_array()).unwrap();
    
    let mut asset_ids = Vec::new();
    for t in tokens {
        if let Some(tid) = t.get("token_id").and_then(|v| v.as_str()) {
            asset_ids.push(tid.to_string());
        }
    }
    
    println!("✅ Found Market: {}", question);
    println!("🔑 Asset IDs: {:?}", asset_ids);
    
    // 2. Connect WS
    let ws_url = "wss://ws-subscriptions-clob.polymarket.com/ws/market";
    println!("🔌 Connecting to WS: {}", ws_url);
    
    let (ws_stream, _) = connect_async(ws_url).await?;
    println!("✅ Connected!");
    
    let (mut write, mut read) = ws_stream.split();
    
    // 3. Subscribe
    // Try "assets_ids" format
    let sub_msg = serde_json::json!({
        "assets_ids": asset_ids,
        "type": "market"
    });
    
    let sub_str = sub_msg.to_string();
    println!("Tx: {}", sub_str);
    write.send(Message::Text(sub_str)).await?;
    
    // 4. Read Loop
    println!("👂 Listening for messages (Press Ctrl+C to stop)...");
    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(txt)) => {
                println!("📩 Rx: {}", txt);
            },
            Ok(Message::Ping(_)) => {
                println!("🏓 Ping");
            },
            Ok(Message::Close(_)) => {
                println!("❌ Closed");
                break;
            },
            Err(e) => {
                println!("❌ Error: {}", e);
                break;
            },
            _ => {}
        }
    }
    
    Ok(())
}
