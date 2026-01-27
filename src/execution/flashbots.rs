use ethers::prelude::*;
use ethers::types::transaction::eip2718::TypedTransaction;
use ethers_flashbots::{FlashbotsMiddleware, BundleRequest};
use std::sync::Arc;
use url::Url;
use anyhow::Result;
use tracing::{info, error};

// Define type aliases to keep sanity
type HttpProvider = Provider<Http>;
type Wallet = LocalWallet;
type SignerClient = SignerMiddleware<HttpProvider, Wallet>;
// FlashbotsMiddleware wraps the SignerClient directly, not Arc<SignerClient> usually, unless clone needed.
// But we want to share it. 
// Let's try wrapping the whole thing in a Box<dyn ...> or just keeping the long type but correct.
// The error was likely due to `Arc<SignerClient>` trait bounds.
// Let's hold `Arc<FlashbotsMiddleware<SignerClient, Wallet>>`.

type FlashbotsClientType = FlashbotsMiddleware<SignerClient, Wallet>;

pub struct FlashbotsClient {
    client: Option<Arc<FlashbotsClientType>>,
}

impl FlashbotsClient {
    pub async fn new(
        rpc_url: &str, 
        private_key: &str, 
        flashbots_relay_url: Option<&str>
    ) -> Result<Self> {
        let provider = Provider::<Http>::try_from(rpc_url)?;
        let wallet: LocalWallet = private_key.parse()?;
        let chain_id = provider.get_chainid().await?.as_u64();
        let wallet = wallet.with_chain_id(chain_id);
        
        // FlashbotsMiddleware takes M, S.
        let signer_middleware = SignerMiddleware::new(provider, wallet.clone());

        let relay_url = Url::parse(flashbots_relay_url.unwrap_or("https://relay.flashbots.net"))?;

        // Random signer for the relay identification
        let bundle_signer = LocalWallet::new(&mut rand::thread_rng());

        let client = FlashbotsMiddleware::new(
            signer_middleware,
            relay_url,
            bundle_signer
        );

        Ok(Self {
            client: Some(Arc::new(client)),
        })
    }

    pub async fn send_private_tx(&self, tx: Eip1559TransactionRequest) -> Result<H256> {
        if let Some(client) = &self.client {
            info!("🤫 Sending PRIVATE transaction via Flashbots/Relay...");
            
            // 1. Convert to TypedTransaction
            let mut typed_tx: TypedTransaction = tx.into();
            
            // 2. Fill transaction (gas, nonce) using inner provider
            client.inner().fill_transaction(&mut typed_tx, None).await?;
            
            // 3. Sign the transaction
            // The inner middleware is SignerMiddleware, so we can sign
            let signature = client.inner().signer().sign_transaction(&typed_tx).await?;
            let signed_tx = typed_tx.rlp_signed(&signature);
            
            // 4. Create Bundle
            let mut bundle = BundleRequest::new();
            bundle = bundle.push_transaction(signed_tx);
            
            // 5. Send Bundle
            let pending_bundle = client.send_bundle(&bundle).await?;
            
            let bundle_hash = pending_bundle.bundle_hash.unwrap_or(H256::zero());
            
            info!("📦 Bundle sent! Hash: {:?}", bundle_hash);
            Ok(bundle_hash)
        } else {
            anyhow::bail!("Flashbots client not initialized");
        }
    }
}
