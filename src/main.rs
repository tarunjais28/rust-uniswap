use futures::StreamExt;
use num_bigint::BigInt;
use num_traits::{One, ToPrimitive};
use std::{collections::HashMap, str::FromStr};
use web3::{
    ethabi::{Address, Event, Int, Log},
    transports::WebSocket,
    types::{H160, H256, U64},
    Web3,
};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    const WEBSOCKET_INFURA_ENDPOINT: &str =
        "wss://mainnet.infura.io/ws/v3/e29db00fcc5142e993209387d6219168";

    let web3 =
        web3::Web3::new(web3::transports::ws::WebSocket::new(WEBSOCKET_INFURA_ENDPOINT).await?);
    let contract_address = web3::types::H160::from_slice(
        &hex::decode("5777d92f208679db4b9778590fa3cab3ac9e2168").unwrap()[..],
    );
    let contract = web3::contract::Contract::from_json(
        web3.eth(),
        contract_address,
        include_bytes!("contracts/uniswap_pool_abi.json"),
    )?;
    let swap_event = contract.abi().events_by_name("Swap")?.first().unwrap();
    let swap_event_signature = swap_event.signature();

    let mut block_stream = web3.eth_subscribe().subscribe_new_heads().await?;
    let mut data: HashMap<U64, BlockData> = HashMap::new();

    while let Some(Ok(block)) = block_stream.next().await {
        let block_number = block.number.expect("Error getting block number!");
        let block_hash = block.hash.expect("Error getting block hash!");

        println!("current block: {}", block_number);

        read_and_add_logs(
            web3.clone(),
            block_hash,
            contract_address,
            swap_event_signature,
            &mut data,
            block_number,
        )
        .await?;

        // Showing blocks N - 5 to protect against reorganisation
        show(
            web3.clone(),
            contract_address,
            swap_event,
            swap_event_signature,
            &mut data,
            block_number - 5,
        )
        .await?;

        check_for_reorganization(
            &mut data,
            block_number,
            web3.clone(),
            contract_address,
            swap_event_signature,
        )
        .await?;
    }

    Ok(())
}

struct BlockData {
    block_hash: H256,
    log: Vec<web3::types::Log>,
}

async fn read_logs(
    web3: Web3<WebSocket>,
    block_hash: H256,
    contract_address: H160,
    swap_event_signature: H256,
) -> Result<Vec<web3::types::Log>, anyhow::Error> {
    let logs = web3
        .eth()
        .logs(
            web3::types::FilterBuilder::default()
                .block_hash(block_hash)
                .address(vec![contract_address])
                .topics(Some(vec![swap_event_signature]), None, None, None)
                .build(),
        )
        .await?;

    Ok(logs)
}

async fn read_and_add_logs(
    web3: Web3<WebSocket>,
    block_hash: H256,
    contract_address: H160,
    swap_event_signature: H256,
    data: &mut HashMap<U64, BlockData>,
    block_number: U64,
) -> Result<Vec<web3::types::Log>, anyhow::Error> {
    let logs = read_logs(web3, block_hash, contract_address, swap_event_signature).await?;

    if !logs.is_empty() {
        data.insert(
            block_number,
            BlockData {
                block_hash,
                log: logs.clone(),
            },
        );
    }

    Ok(logs)
}

async fn show(
    web3: Web3<WebSocket>,
    contract_address: H160,
    swap_event: &Event,
    swap_event_signature: H256,
    data: &mut HashMap<U64, BlockData>,
    block_number: U64,
) -> Result<(), anyhow::Error> {
    if let Some(block_data) = data.get(&block_number) {
        println!("show block: {}", block_number);

        let swap_logs_in_block = read_logs(
            web3,
            block_data.block_hash,
            contract_address,
            swap_event_signature,
        )
        .await?;

        for log in swap_logs_in_block {
            let parsed_log = swap_event.parse_log(web3::ethabi::RawLog {
                topics: log.topics,
                data: log.data.0,
            })?;
            println!("{:#?}", get_output_field(parsed_log));
        }
    };

    Ok(())
}

#[derive(Default, Debug)]
struct Output {
    sender: Address,
    recipient: Address,
    dai: f64,
    usdc: f64,
    direction: String,
}

impl Output {
    fn new() -> Self {
        Self::default()
    }
}

fn get_output_field(log: Log) -> Output {
    let mut output = Output::new();
    let mut dai = Int::max_value();
    let mut usdc = Int::max_value();

    for param in log.params {
        match param.name.as_str() {
            "sender" => output.sender = param.value.into_address().unwrap(),
            "recipient" => output.recipient = param.value.into_address().unwrap(),
            "amount0" => dai = param.value.into_int().unwrap(),
            "amount1" => usdc = param.value.into_int().unwrap(),
            _ => (),
        }
    }

    if dai > usdc {
        output.dai = twos_complement(&dai).to_f64().unwrap() / 1_000000_000000_000000.0f64;
        output.usdc = (usdc.as_u128() as f64) / 1_000000.0f64;
        output.direction = String::from("USDC -> DAI");
    } else {
        output.usdc = twos_complement(&usdc).to_f64().unwrap() / 1_000000.0f64;
        output.dai = (dai.as_u128() as f64) / 1_000000_000000_000000.0f64;
        output.direction = String::from("DAI -> USDC");
    }

    output
}

fn twos_complement(value: &Int) -> BigInt {
    // Convert web3::ethabi::Int to num_bigint::BigInt
    let big_int_value = BigInt::from_str(&value.to_string()).unwrap();

    // Calculate 2^bit_size
    let two_power = BigInt::one() << 256;

    // Calculate two's complement
    &two_power - big_int_value
}

async fn check_for_reorganization(
    data: &mut HashMap<U64, BlockData>,
    block_number: U64,
    web3: Web3<WebSocket>,
    contract_address: H160,
    swap_event_signature: H256,
) -> Result<(), anyhow::Error> {
    for (block, block_data) in data {
        // Skipping for first N + 5 depth
        let depth_opt = block_number.checked_sub(block + 5);

        if let Some(depth) = depth_opt {
            if depth > U64::zero() {
                let logs = read_logs(
                    web3.clone(),
                    block_data.block_hash,
                    contract_address,
                    swap_event_signature,
                )
                .await?;

                anyhow::ensure!(
                    logs.eq(&block_data.log),
                    "Error: Reorganization happen after depth 5!"
                );
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
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
}
