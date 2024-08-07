use super::*;
use hex_literal::hex;

#[tokio::test]
async fn test_for_reorganization_with_empty_data_map() {
    let endpoint = "wss://mainnet.infura.io/ws/v3/e29db00fcc5142e993209387d6219168";
    let mut data: HashMap<U64, BlockData> = HashMap::new();
    let block_number = U64::from(4);
    let web3 = web3::Web3::new(
        web3::transports::ws::WebSocket::new(endpoint)
            .await
            .unwrap(),
    );
    let contract_address = H160::from_low_u64_be(1);
    let swap_event_signature = H256::from_low_u64_be(2);

    assert!(check_for_reorganization(
        &mut data,
        block_number,
        web3,
        contract_address,
        swap_event_signature,
    )
    .await
    .is_ok());
}

#[tokio::test]
async fn test_for_reorganization_with_depth_lesser_than_5() {
    let endpoint = "wss://mainnet.infura.io/ws/v3/e29db00fcc5142e993209387d6219168";

    let log = web3::types::Log {
        address: Address::from_low_u64_be(1),
        topics: vec![],
        data: hex!("").into(),
        block_hash: Some(H256::from_low_u64_be(2)),
        block_number: Some(1.into()),
        transaction_hash: Some(H256::from_low_u64_be(3)),
        transaction_index: Some(0.into()),
        log_index: Some(0.into()),
        transaction_log_index: Some(0.into()),
        log_type: Some("removed".into()),
        removed: None,
    };

    let mut data: HashMap<U64, BlockData> = HashMap::new();
    data.insert(
        log.block_number.unwrap(),
        BlockData {
            block_hash: log.block_hash.unwrap(),
            log: vec![log],
        },
    );

    let block_number = U64::from(4);
    let web3 = web3::Web3::new(
        web3::transports::ws::WebSocket::new(endpoint)
            .await
            .unwrap(),
    );
    let contract_address = H160::from_low_u64_be(1);
    let swap_event_signature = H256::from_low_u64_be(2);

    assert!(check_for_reorganization(
        &mut data,
        block_number,
        web3,
        contract_address,
        swap_event_signature,
    )
    .await
    .is_ok());
}

#[tokio::test]
async fn test_for_reorganization_with_depth_equal_to_5() {
    let endpoint = "wss://mainnet.infura.io/ws/v3/e29db00fcc5142e993209387d6219168";

    let log = web3::types::Log {
        address: Address::from_low_u64_be(1),
        topics: vec![],
        data: hex!("").into(),
        block_hash: Some(H256::from_low_u64_be(2)),
        block_number: Some(6.into()),
        transaction_hash: Some(H256::from_low_u64_be(3)),
        transaction_index: Some(0.into()),
        log_index: Some(0.into()),
        transaction_log_index: Some(0.into()),
        log_type: Some("removed".into()),
        removed: None,
    };

    let mut data: HashMap<U64, BlockData> = HashMap::new();
    data.insert(
        log.block_number.unwrap(),
        BlockData {
            block_hash: log.block_hash.unwrap(),
            log: vec![log],
        },
    );

    let block_number = U64::from(5);
    let web3 = web3::Web3::new(
        web3::transports::ws::WebSocket::new(endpoint)
            .await
            .unwrap(),
    );
    let contract_address = H160::from_low_u64_be(1);
    let swap_event_signature = H256::from_low_u64_be(2);

    assert!(check_for_reorganization(
        &mut data,
        block_number,
        web3,
        contract_address,
        swap_event_signature,
    )
    .await
    .is_ok());
}
