use std::{collections::HashMap, sync::Arc};

use alloy::{
    dyn_abi::DynSolType, network::Network, primitives::Address, providers::Provider, sol,
    transports::Transport,
};
use futures::{stream::FuturesUnordered, StreamExt};
use serde::{Deserialize, Serialize};

pub mod amm;
pub mod balancer;
pub mod consts;
pub mod erc_4626;
pub mod error;
pub mod factory;
pub mod float;
pub mod uniswap_v2;
pub mod uniswap_v3;

sol! {
    #[sol(rpc)]
    GetTokenDecimalsBatchRequest,
    "contracts/out/GetTokenDecimalsBatchRequest.sol/GetTokenDecimalsBatchRequest.json",
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Token {
    address: Address,
    decimals: u8,
    // TODO: add optional tax
}

impl Token {
    pub const fn new(address: Address, decimals: u8) -> Self {
        Self { address, decimals }
    }

    pub const fn address(&self) -> Address {
        self.address
    }

    pub const fn decimals(&self) -> u8 {
        self.decimals
    }
}

/// Fetches the decimal precision for a list of ERC-20 tokens.
///
/// # Returns
/// A map of token addresses to their decimal precision.
pub async fn get_token_decimals<T, N, P>(
    tokens: Vec<Address>,
    provider: Arc<P>,
) -> HashMap<Address, u8>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N>,
{
    let step = 765;

    let mut futures = FuturesUnordered::new();
    tokens.chunks(step).for_each(|group| {
        let provider = provider.clone();

        futures.push(async move {
            (
                group,
                GetTokenDecimalsBatchRequest::deploy_builder(provider, group.to_vec())
                    .call_raw()
                    .await
                    .expect("TODO: handle error"),
            )
        });
    });

    let mut token_decimals = HashMap::new();
    let return_type = DynSolType::Array(Box::new(DynSolType::Uint(8)));

    while let Some(res) = futures.next().await {
        let (token_addresses, return_data) = res;

        let return_data = return_type
            .abi_decode_sequence(&return_data)
            .expect("TODO: handle error");

        if let Some(tokens_arr) = return_data.as_array() {
            for (decimals, token_address) in tokens_arr.iter().zip(token_addresses.iter()) {
                token_decimals.insert(
                    *token_address,
                    decimals.as_uint().expect("TODO:").0.to::<u8>(),
                );
            }
        }
    }
    token_decimals
}
