use polyfill_rs::ClobClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Private key
    let private_key = "here";
    
    println!("🔑 Deriving Polymarket API credentials from private key...\n");
    
    // Initialize CLOB client with host and chain ID
    let host = "https://clob.polymarket.com";
    let chain_id = 137; // Polygon mainnet
    
    let client = ClobClient::with_l1_headers(host, private_key, chain_id);
    
    // Derive API credentials using L1 auth
    let creds = client.create_or_derive_api_key(None).await?;
    
    println!("✅ API Credentials derived successfully!\n");
    println!("Copy these values to your .env file:\n");
    println!("POLYMARKET_API_KEY={}", creds.api_key);
    println!("POLYMARKET_SECRET={}", creds.secret);
    println!("POLYMARKET_PASSPHRASE={}", creds.passphrase);
    println!("\n⚠️  Keep these credentials secure and never commit them to git!");
    
    Ok(())
}
