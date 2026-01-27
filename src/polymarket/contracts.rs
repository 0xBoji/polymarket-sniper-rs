use ethers::prelude::*;

abigen!(
    CtfExchange,
    r#"[
        function buy(bytes32 conditionId, uint256 outcomeIndex, uint256 amount) external
        function fillOrder(tuple(uint256 salt, address maker, address signer, address taker, uint256 tokenId, uint256 makerAmount, uint256 takerAmount, uint256 expiration, uint256 nonce, uint256 feeRateBps, uint8 side, uint8 signatureType, bytes signature) order, uint256 fillAmount) external
    ]"#
);
