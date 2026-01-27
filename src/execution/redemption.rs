use anyhow::Result;
use ethers::prelude::*;
use ethers::types::Address;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{info, warn, error};

// Partial ABI for Conditional Tokens Framework (CTF)
abigen!(
    CTF,
    r#"[
        function payoutDenominator(bytes32 conditionId) external view returns (uint256)
        function payoutNumerators(bytes32 conditionId, uint256 index) external view returns (uint256)
        function redeemPositions(address collateralToken, bytes32 parentCollectionId, bytes32 conditionId, uint256[] calldata indexSets) external
    ]"#
);

const CTF_ADDRESS: &str = "0x4d97dcd97ec945f40cf65f87097ace5ea0476045";
const COLLATERAL_TOKEN: &str = "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174"; // USDC.e on Polygon

pub struct RedemptionManager {
    contract: CTF<Provider<Ws>>, // Using WebSocket provider
    #[allow(dead_code)]
    wallet: Option<LocalWallet>, // Wallet for signing transactions (optional for read-only checks)
    client: Arc<SignerMiddleware<Provider<Ws>, LocalWallet>>,
}

impl RedemptionManager {
    /// Initialize RedemptionManager with WS RPC URL and Private Key
    pub async fn new(rpc_url: &str, private_key: &str) -> Result<Self> {
        let provider = Provider::<Ws>::connect(rpc_url).await?;
        let wallet = LocalWallet::from_str(private_key)?.with_chain_id(137u64); // Polygon Mainnet ID
        
        // Use clone for client (SignerMiddleware takes the provider instance)
        let client = Arc::new(SignerMiddleware::new(provider.clone(), wallet.clone()));
        
        // Parse address
        let address = Address::from_str(CTF_ADDRESS)?;
        
        // Wrap provider in Arc for CTF contract (read-only instance)
        // CTF::new expects Into<Arc<M>>
        let contract = CTF::new(address, Arc::new(provider)); 

        Ok(Self {
            contract,
            wallet: Some(wallet),
            client,
        })
    }

    /// Check if a condition is resolved using payoutDenominator
    pub async fn is_condition_resolved(&self, condition_id_hex: &str) -> Result<bool> {
        let condition_id = self.parse_bytes32(condition_id_hex)?;
        
        // payoutDenominator > 0 means resolved
        let denominator = self.contract.payout_denominator(condition_id).call().await?;
        
        Ok(denominator > U256::zero())
    }

    /// Redeem positions for a resolved condition
    /// For binary markets: indexSets = [1, 2] usually (Outcome A and Outcome B)
    pub async fn redeem_positions(&self, condition_id_hex: &str) -> Result<String> {
        info!("💰 Attempting to redeem positions for {}", condition_id_hex);

        let condition_id = self.parse_bytes32(condition_id_hex)?;
        let parent_collection_id = [0u8; 32]; // Always 0x0 for direct questions
        let collateral_token = Address::from_str(COLLATERAL_TOKEN)?;
        
        // Index sets for binary market (1 and 2)
        // 1 = 0b01 (Outcome 0), 2 = 0b10 (Outcome 1)
        let index_sets = vec![U256::from(1), U256::from(2)];

        // We need to use the client with signer to send tx
        let address = Address::from_str(CTF_ADDRESS)?;
        let contract_with_signer = CTF::new(address, self.client.clone());

        let tx = contract_with_signer
            .redeem_positions(
                collateral_token,
                parent_collection_id,
                condition_id,
                index_sets,
            );

        // Send transaction
        let pending_tx = tx.send().await?;
        let tx_hash = pending_tx.tx_hash();
        
        info!("✅ Redeem transaction sent! Hash: {:?}", tx_hash);
        
        // Wait for receipt (optional, maybe don't block)
        // let receipt = pending_tx.await?;
        
        Ok(format!("{:?}", tx_hash))
    }

    fn parse_bytes32(&self, hex_str: &str) -> Result<[u8; 32]> {
        let clean = hex_str.trim_start_matches("0x");
        let bytes = hex::decode(clean)?;
        if bytes.len() != 32 {
            anyhow::bail!("Invalid condition ID length");
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(arr)
    }
}
