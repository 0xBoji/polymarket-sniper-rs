use ethers::prelude::*;
use std::str::FromStr;
use anyhow::Result;

/*
abigen!(
    CtfExchange,
    r#"[
        function buy(bytes32 conditionId, uint256 outcomeIndex, uint256 amount) external
        function fillOrder(tuple(uint256 salt, address maker, address signer, address taker, uint256 tokenId, uint256 makerAmount, uint256 takerAmount, uint256 expiration, uint256 nonce, uint256 feeRateBps, uint8 side, uint8 signatureType, bytes signature) order, uint256 fillAmount) external
    ]"#
);
*/

/// Derives the Asset IDs (Token IDs) for a given Condition ID for a binary market.
/// Assumes standard Polymarket configuration:
/// - Collateral: USDC (Polygon)
/// - Parent Collection: 0x0
/// - Binary outcomes: Index 0 (NO), Index 1 (YES)
pub fn derive_asset_ids(condition_id_str: &str) -> Result<(String, String)> {
    // 1. Constants
    // USDC on Polygon
    let collateral_token = Address::from_str("0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174")?;
    let parent_collection_id = H256::zero();
    let condition_id = H256::from_str(condition_id_str)?;

    // 2. Define Index Sets for Binary Market
    // NO = Index 0 => 1 << 0 = 1
    // YES = Index 1 => 1 << 1 = 2
    let index_set_no = U256::from(1);
    let index_set_yes = U256::from(2);

    // 3. Helper for Gnosis CTF Hashing
    // CollectionId = keccak256(abi.encodePacked(parentCollectionId, conditionId, indexSet))
    fn get_collection_id(parent: H256, condition: H256, index_set: U256) -> H256 {
        let mut encoded = Vec::new();
        encoded.extend_from_slice(parent.as_bytes());
        encoded.extend_from_slice(condition.as_bytes());
        
        // index_set is uint256 (32 bytes)
        let mut index_bytes = [0u8; 32];
        index_set.to_big_endian(&mut index_bytes);
        encoded.extend_from_slice(&index_bytes);
        
        H256::from(ethers::utils::keccak256(&encoded))
    }

    // PositionId = keccak256(abi.encodePacked(collateralToken, collectionId))
    fn get_position_id(collateral: Address, collection: H256) -> String {
        let mut encoded = Vec::new();
        encoded.extend_from_slice(collateral.as_bytes());
        encoded.extend_from_slice(collection.as_bytes());
        
        // Return as massive decimal string (Token ID) usually?
        // Wait, Polymarket CLOB uses the DECIMAL STRING representation of the uint256 Position ID.
        // OR does it use the Hex string?
        // Checking API: "token_id": "217426331434639062905690501558262415339047831329679843560126160201174249"
        // It uses the DECIMAL string.
        
        let hash = H256::from(ethers::utils::keccak256(&encoded));
        let params = U256::from_big_endian(hash.as_bytes());
        params.to_string()
    }
    
    // 4. Compute
    let collection_no = get_collection_id(parent_collection_id, condition_id, index_set_no);
    let collection_yes = get_collection_id(parent_collection_id, condition_id, index_set_yes);
    
    let asset_id_no = get_position_id(collateral_token, collection_no);
    let asset_id_yes = get_position_id(collateral_token, collection_yes);
    
    Ok((asset_id_yes, asset_id_no))
}
