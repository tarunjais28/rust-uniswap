use super::*;

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

pub async fn read_and_add_logs(
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

pub async fn show(
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
            println!("{:#?}", get_output_fields(parsed_log));
        }
    };

    Ok(())
}

fn get_output_fields(log: Log) -> Output {
    let mut output = Output::new();
    let mut dai = Int::max_value();
    let mut usdc = Int::max_value();

    for param in log.params {
        match param.name.as_str() {
            "sender" => {
                output.sender = param
                    .value
                    .into_address()
                    .expect("Error while getting sender!")
            }
            "recipient" => {
                output.recipient = param
                    .value
                    .into_address()
                    .expect("Error while getting recipient!")
            }
            "amount0" => {
                dai = param
                    .value
                    .into_int()
                    .expect("Error while getting amount0!")
            }
            "amount1" => {
                usdc = param
                    .value
                    .into_int()
                    .expect("Error while getting amount1!")
            }
            _ => (),
        }
    }

    if dai > usdc {
        output.dai = twos_complement(&dai)
            .to_f64()
            .expect("Error while converting dai to floating point number!")
            / 1_000000_000000_000000.0f64;
        output.usdc = (usdc.as_u128() as f64) / 1_000000.0f64;
        output.direction = String::from("USDC -> DAI");
    } else {
        output.usdc = twos_complement(&usdc)
            .to_f64()
            .expect("Error while converting usdc to floating point number!")
            / 1_000000.0f64;
        output.dai = (dai.as_u128() as f64) / 1_000000_000000_000000.0f64;
        output.direction = String::from("DAI -> USDC");
    }

    output
}

fn twos_complement(value: &Int) -> BigInt {
    // Convert web3::ethabi::Int to num_bigint::BigInt
    let big_int_value =
        BigInt::from_str(&value.to_string()).expect("Error while converting value to Bigint!");

    // Calculate 2^bit_size
    let two_power = BigInt::one() << 256;

    // Calculate two's complement
    &two_power - big_int_value
}

pub async fn check_for_reorganization(
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
