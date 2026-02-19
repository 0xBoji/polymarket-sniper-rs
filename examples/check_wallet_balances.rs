use ethers::prelude::*;
use polymarket_client_sdk::types::Address as PolyAddress;
use polymarket_client_sdk::{derive_proxy_wallet, derive_safe_wallet, POLYGON};
use std::str::FromStr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let pk = std::env::var("POLYGON_PRIVATE_KEY")?;
    let wallet = LocalWallet::from_str(&pk)?.with_chain_id(POLYGON);
    let eoa = wallet.address();
    let poly_eoa = PolyAddress::from(eoa.0);

    let safe = derive_safe_wallet(poly_eoa, POLYGON)
        .map(|a| Address::from_str(&a.to_string()))
        .transpose()?;
    let proxy = derive_proxy_wallet(poly_eoa, POLYGON)
        .map(|a| Address::from_str(&a.to_string()))
        .transpose()?;
    let env_proxy = std::env::var("POLYMARKET_PROXY_ADDRESS")
        .ok()
        .and_then(|s| Address::from_str(&s).ok());

    let provider = Provider::<Http>::try_from("https://polygon-rpc.com")?;

    abigen!(
        ERC20,
        r#"[function balanceOf(address account) external view returns (uint256)]"#
    );
    let usdc_native = ERC20::new(
        Address::from_str("0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359")?,
        provider.clone().into(),
    );
    let usdc_bridged = ERC20::new(
        Address::from_str("0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174")?,
        provider.clone().into(),
    );

    let mut wallets: Vec<(&str, Address)> = vec![("EOA", eoa)];
    if let Some(addr) = safe {
        wallets.push(("SAFE(derived)", addr));
    }
    if let Some(addr) = proxy {
        wallets.push(("PROXY(derived)", addr));
    }
    if let Some(addr) = env_proxy {
        wallets.push(("ENV_PROXY", addr));
    }

    wallets.sort_by_key(|(_, a)| a.to_string());
    wallets.dedup_by_key(|(_, a)| *a);

    println!("EOA: {}", eoa);
    if let Some((_, a)) = wallets.iter().find(|(k, _)| *k == "SAFE(derived)") {
        println!("SAFE(derived): {}", a);
    }
    if let Some((_, a)) = wallets.iter().find(|(k, _)| *k == "PROXY(derived)") {
        println!("PROXY(derived): {}", a);
    }
    if let Some((_, a)) = wallets.iter().find(|(k, _)| *k == "ENV_PROXY") {
        println!("ENV_PROXY: {}", a);
    }
    println!();

    for (label, addr) in wallets {
        let native = usdc_native.balance_of(addr).call().await?;
        let bridged = usdc_bridged.balance_of(addr).call().await?;
        let native_f = native.as_u128() as f64 / 1_000_000.0;
        let bridged_f = bridged.as_u128() as f64 / 1_000_000.0;
        println!(
            "{:<14} {} | USDC(native): ${:>10.4} | USDC.e: ${:>10.4}",
            label, addr, native_f, bridged_f
        );
    }

    Ok(())
}
