use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use alloy::{
    dyn_abi::DynSolType,
    network::Network,
    primitives::{address, bytes, Address, Bytes, U256},
    providers::Provider,
    sol,
    transports::Transport,
};
use eyre::Result;

pub const MULTICALL_ADDRESS: Address = address!("0000000000002Bdbf1Bf3279983603Ec279CC6dF");
pub const DECIMALS_SELECTOR: Bytes = bytes!("313ce567");

sol! {
    #[sol(rpc)]
    contract Multicall {
        function aggregate(
            address[] calldata targets,
            bytes[] calldata data,
            uint256[] calldata values,
            address refundTo
        ) external returns (bytes[] memory);
    }
}

pub async fn populate_token_decimals<T, N, P>(
    tokens: HashSet<Address>,
    provider: Arc<P>,
) -> Result<HashMap<Address, u8>>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N>,
{
    let data = vec![DECIMALS_SELECTOR; tokens.len()];
    let values = vec![U256::ZERO; tokens.len()];
    let multicaller = Multicall::new(MULTICALL_ADDRESS, provider.clone());
    let res = multicaller
        .aggregate(Vec::from_iter(tokens.clone()), data, values, Address::ZERO)
        .call()
        .await?
        ._0;

    Ok(tokens
        .iter()
        .cloned()
        .zip(res.into_iter())
        .map(|(token, res)| {
            let decimals: u8 = DynSolType::Uint(8)
                .abi_decode(&res)
                .unwrap()
                .as_uint()
                .unwrap_or_default()
                .0
                .to();
            (token, decimals)
        })
        .collect::<HashMap<_, _>>())
}
