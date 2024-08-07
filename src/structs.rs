use super::*;

pub struct BlockData {
    pub block_hash: H256,
    pub log: Vec<web3::types::Log>,
}

#[derive(Default, Debug)]
pub struct Output {
    pub sender: Address,
    pub recipient: Address,
    pub dai: f64,
    pub usdc: f64,
    pub direction: String,
}

impl Output {
    pub fn new() -> Self {
        Self::default()
    }
}
