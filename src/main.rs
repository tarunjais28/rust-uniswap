use crate::{helper::*, structs::*};
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

mod helper;
mod structs;
#[cfg(test)]
mod tests;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    const WEBSOCKET_INFURA_ENDPOINT: &str =
        "wss://mainnet.infura.io/ws/v3/e29db00fcc5142e993209387d6219168";

    let web3 =
        web3::Web3::new(web3::transports::ws::WebSocket::new(WEBSOCKET_INFURA_ENDPOINT).await?);
    let contract_address = web3::types::H160::from_slice(
        &hex::decode("5777d92f208679db4b9778590fa3cab3ac9e2168")
            .expect("Error while decoding to hex!")[..],
    );
    let contract = web3::contract::Contract::from_json(
        web3.eth(),
        contract_address,
        include_bytes!("contracts/uniswap_pool_abi.json"),
    )?;
    let swap_event = contract
        .abi()
        .events_by_name("Swap")?
        .first()
        .expect("Error while getting event by name!");
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
