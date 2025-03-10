use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::Arc,
};

use alloy::{
    dyn_abi::DynSolType, network::Network, primitives::Address, providers::Provider, sol,
};
use error::AMMError;
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

sol!(
#[derive(Debug, PartialEq, Eq)]
#[sol(rpc)]
contract IERC20 {
    function decimals() external view returns (uint8);
});

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Token {
    pub address: Address,
    pub decimals: u8,
    // TODO: add optional tax
}

impl Token {
    pub async fn new<N, P>(address: Address, provider: P) -> Result<Self, AMMError>
    where
        N: Network,
        P: Provider<N>,
    {
        let decimals = IERC20::new(address, provider).decimals().call().await?._0;

        Ok(Self { address, decimals })
    }

    pub const fn new_with_decimals(address: Address, decimals: u8) -> Self {
        Self { address, decimals }
    }

    pub const fn address(&self) -> &Address {
        &self.address
    }

    pub const fn decimals(&self) -> u8 {
        self.decimals
    }
}

impl From<Address> for Token {
    fn from(address: Address) -> Self {
        Self {
            address,
            decimals: 0,
        }
    }
}

impl Hash for Token {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.address.hash(state);
    }
}

/// Fetches the decimal precision for a list of ERC-20 tokens.
///
/// # Returns
/// A map of token addresses to their decimal precision.
pub async fn get_token_decimals<N, P>(
    tokens: Vec<Address>,
    provider: Arc<P>,
) -> HashMap<Address, u8>
where
    N: Network,
    P: Provider<N>,
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
