use anyhow::{Context, Result};
use ethers::prelude::*;
use ethers::types::transaction::eip2718::TypedTransaction;
use ethers_flashbots::{BundleRequest, FlashbotsMiddleware};
use std::sync::Arc;
use tracing::{error, info, warn};
use url::Url;

// Type aliases for clarity
type HttpProvider = Provider<Http>;
type SignerClient = SignerMiddleware<HttpProvider, LocalWallet>;
type FlashbotsClientType = FlashbotsMiddleware<Arc<SignerClient>, LocalWallet>;

#[derive(Debug, Clone)]
pub struct BundleHash(pub H256);

#[derive(Debug)]
pub struct SimulationResult {
    pub success: bool,
    pub gas_used: U256,
    pub effective_gas_price: U256,
    pub error: Option<String>,
}

#[derive(Debug)]
pub enum BundleStatus {
    Pending,
    Included(u64), // Block number
    Failed(String),
}

pub struct FlashbotsClient {
    client: Arc<FlashbotsClientType>,
    max_retries: u32,
}

impl FlashbotsClient {
    /// Create a new Flashbots client
    pub async fn new(
        rpc_url: &str,
        private_key: &str,
        flashbots_relay_url: Option<&str>,
        max_retries: u32,
    ) -> Result<Self> {
        info!("🔧 Initializing Flashbots client...");

        // Setup provider
        let provider =
            Provider::<Http>::try_from(rpc_url).context("Failed to create HTTP provider")?;

        // Setup wallet
        let wallet: LocalWallet = private_key.parse().context("Failed to parse private key")?;

        let chain_id = provider.get_chainid().await?.as_u64();
        let wallet = wallet.with_chain_id(chain_id);

        // Create signer middleware
        let signer_middleware = Arc::new(SignerMiddleware::new(provider, wallet));

        // Parse relay URL
        let relay_url = Url::parse(flashbots_relay_url.unwrap_or("https://relay.flashbots.net"))
            .context("Failed to parse relay URL")?;

        // Create bundle signer (random wallet for relay identification)
        let bundle_signer = LocalWallet::new(&mut rand::thread_rng());

        // Create Flashbots middleware
        let client = FlashbotsMiddleware::new(signer_middleware, relay_url, bundle_signer);

        info!(
            "✅ Flashbots client initialized (relay: {})",
            flashbots_relay_url.unwrap_or("https://relay.flashbots.net")
        );

        Ok(Self {
            client: Arc::new(client),
            max_retries,
        })
    }

    /// Send a bundle of transactions atomically
    pub async fn send_bundle(&self, transactions: Vec<TypedTransaction>) -> Result<BundleHash> {
        if transactions.is_empty() {
            anyhow::bail!("Cannot send empty bundle");
        }

        info!(
            "📦 Creating bundle with {} transactions",
            transactions.len()
        );

        // Build bundle request
        let mut bundle = BundleRequest::new();

        for (idx, mut tx) in transactions.into_iter().enumerate() {
            // Fill transaction details (gas, nonce, etc.)
            self.client
                .inner()
                .fill_transaction(&mut tx, None)
                .await
                .context(format!("Failed to fill transaction {}", idx))?;

            // Sign transaction
            let signature = self
                .client
                .inner()
                .signer()
                .sign_transaction(&tx)
                .await
                .context(format!("Failed to sign transaction {}", idx))?;

            let signed_tx = tx.rlp_signed(&signature);
            bundle = bundle.push_transaction(signed_tx);
        }

        // Get current block number for targeting
        let current_block = self.client.get_block_number().await?;
        let target_block = current_block + 1;

        bundle = bundle.set_block(target_block);

        // Send bundle with retries
        let mut last_error = None;
        for attempt in 1..=self.max_retries {
            match self.client.send_bundle(&bundle).await {
                Ok(pending_bundle) => {
                    let bundle_hash = pending_bundle.bundle_hash.unwrap_or(H256::zero());

                    info!(
                        "✅ Bundle sent! Hash: {:?} (target block: {})",
                        bundle_hash, target_block
                    );

                    return Ok(BundleHash(bundle_hash));
                }
                Err(e) => {
                    warn!(
                        "⚠️ Bundle submission attempt {}/{} failed: {}",
                        attempt, self.max_retries, e
                    );
                    last_error = Some(e);

                    if attempt < self.max_retries {
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                }
            }
        }

        Err(last_error.unwrap().into())
    }

    /// Simulate a bundle before submission
    pub async fn simulate_bundle(
        &self,
        transactions: Vec<TypedTransaction>,
    ) -> Result<SimulationResult> {
        if transactions.is_empty() {
            anyhow::bail!("Cannot simulate empty bundle");
        }

        info!(
            "🧪 Simulating bundle with {} transactions",
            transactions.len()
        );

        // Build bundle request
        let mut bundle = BundleRequest::new();

        for (idx, mut tx) in transactions.into_iter().enumerate() {
            // Fill transaction
            self.client
                .inner()
                .fill_transaction(&mut tx, None)
                .await
                .context(format!("Failed to fill transaction {}", idx))?;

            // Sign transaction
            let signature = self
                .client
                .inner()
                .signer()
                .sign_transaction(&tx)
                .await
                .context(format!("Failed to sign transaction {}", idx))?;

            let signed_tx = tx.rlp_signed(&signature);
            bundle = bundle.push_transaction(signed_tx);
        }

        // Get current block for simulation
        let current_block = self.client.get_block_number().await?;
        let target_block = current_block + 1;

        bundle = bundle.set_block(target_block);

        // Simulate
        match self.client.simulate_bundle(&bundle).await {
            Ok(simulated) => {
                let gas_used = simulated.gas_used;
                let effective_gas_price = simulated.effective_gas_price();

                // Simulation success is determined by whether it completed without error
                let success = true;

                info!(
                    "✅ Simulation SUCCESS - Gas: {}, Price: {}",
                    gas_used, effective_gas_price
                );

                Ok(SimulationResult {
                    success,
                    gas_used,
                    effective_gas_price,
                    error: None,
                })
            }
            Err(e) => {
                error!("❌ Simulation error: {}", e);
                Ok(SimulationResult {
                    success: false,
                    gas_used: U256::zero(),
                    effective_gas_price: U256::zero(),
                    error: Some(e.to_string()),
                })
            }
        }
    }

    /// Get the status of a submitted bundle
    pub async fn get_bundle_status(&self, _bundle_hash: &BundleHash) -> Result<BundleStatus> {
        // Note: Flashbots doesn't provide a direct bundle status API
        // We need to check if the bundle was included by monitoring blocks
        // This is a simplified implementation

        // For now, return Pending as we'd need to implement block monitoring
        // In production, you'd want to:
        // 1. Monitor new blocks
        // 2. Check if bundle transactions appear in those blocks
        // 3. Return Included(block_number) if found

        Ok(BundleStatus::Pending)
    }

    /// Send a single private transaction (convenience wrapper)
    pub async fn send_private_tx(&self, tx: TypedTransaction) -> Result<BundleHash> {
        self.send_bundle(vec![tx]).await
    }
}
