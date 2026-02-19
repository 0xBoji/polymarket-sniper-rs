use polyfill_rs::ClobClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = ClobClient::new("https://clob.polymarket.com");

    // I want to see if these methods exist via compiler errors
    let _ = client.create_order("token_id", 1.0, 0.5, "BUY").await;

    println!("Testing ClobClient...");
    Ok(())
}
