use futures::StreamExt;
use num_bigint::BigInt;
use num_traits::{One, ToPrimitive};
use std::str::FromStr;
use web3::ethabi::{Address, Int, Log};

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

    while let Some(Ok(block)) = block_stream.next().await {
        let swap_logs_in_block = web3
            .eth()
            .logs(
                web3::types::FilterBuilder::default()
                    .block_hash(block.hash.unwrap())
                    .address(vec![contract_address])
                    .topics(Some(vec![swap_event_signature]), None, None, None)
                    .build(),
            )
            .await?;

        for log in swap_logs_in_block {
            let parsed_log = swap_event.parse_log(web3::ethabi::RawLog {
                topics: log.topics,
                data: log.data.0,
            })?;
            println!("{:#?}", get_output_field(parsed_log));
        }
    }

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
