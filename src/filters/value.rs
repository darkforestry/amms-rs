use std::sync::Arc;

use alloy::{
    dyn_abi::DynSolType,
    network::Network,
    primitives::{Address, U256},
    providers::Provider,
    sol,
    transports::Transport,
};

use crate::{
    amm::{factory::AutomatedMarketMakerFactory, factory::Factory, AutomatedMarketMaker, AMM},
    errors::AMMError,
};

pub const U256_10_POW_18: U256 = U256::from_limbs([1000000000000000000, 0, 0, 0]);
pub const U256_10_POW_6: U256 = U256::from_limbs([1000000, 0, 0, 0]);

sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    IGetWethValueInAMMBatchRequest,
    "src/filters/batch_requests/GetWethValueInAMMBatchRequest.json"
}

#[allow(clippy::too_many_arguments)]
/// Filter that removes AMMs with less aggregate token value than `usd_value_in_pool_threshold`.
///
/// This function uses batched static calls to get the WETH value in each AMM.
/// Returns a vector of filtered AMMs.
pub async fn filter_amms_below_usd_threshold<T, N, P>(
    amms: Vec<AMM>,
    factories: &[Factory],
    usd_weth_pool: AMM,
    usd_value_in_pool_threshold: f64, // This is the threshold where we will filter out any pool with less value than this
    weth: Address,
    weth_value_in_token_to_weth_pool_threshold: U256, //This is the threshold where we will ignore any token price < threshold during batch calls
    step: usize,
    provider: Arc<P>,
) -> Result<Vec<AMM>, AMMError>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N>,
{
    let weth_usd_price = usd_weth_pool.calculate_price(weth)?;

    // Init a new vec to hold the filtered AMMs
    let mut filtered_amms = vec![];

    let weth_values_in_pools = get_weth_values_in_amms(
        &amms,
        factories,
        weth,
        weth_value_in_token_to_weth_pool_threshold,
        step,
        provider,
    )
    .await?;

    for (i, weth_value) in weth_values_in_pools.iter().enumerate() {
        if (weth_value / U256_10_POW_18).to::<u64>() as f64 * weth_usd_price
            >= usd_value_in_pool_threshold
        {
            // TODO: using clone for now since we only do this once but find a better way in a future update
            filtered_amms.push(amms[i].clone());
        }
    }

    Ok(filtered_amms)
}

/// Filter that removes AMMs with less aggregate token value than `weth_value_in_pool_threshold`.
///
/// This function uses batched static calls to get the WETH value in each AMM.
/// Returns a vector of filtered AMMs.
pub async fn filter_amms_below_weth_threshold<T, N, P>(
    amms: Vec<AMM>,
    factories: &[Factory],
    weth: Address,
    weth_value_in_pool_threshold: U256, // This is the threshold where we will filter out any pool with less value than this
    weth_value_in_token_to_weth_pool_threshold: U256, //This is the threshold where we will ignore any token price < threshold during batch calls
    step: usize,
    provider: Arc<P>,
) -> Result<Vec<AMM>, AMMError>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N>,
{
    let mut filtered_amms = vec![];

    let weth_values_in_pools = get_weth_values_in_amms(
        &amms,
        factories,
        weth,
        weth_value_in_token_to_weth_pool_threshold,
        step,
        provider,
    )
    .await?;

    for (i, weth_value) in weth_values_in_pools.iter().enumerate() {
        if *weth_value >= weth_value_in_pool_threshold {
            // TODO: using clone for now since we only do this once but find a better way in a future update
            filtered_amms.push(amms[i].clone());
        }
    }

    Ok(filtered_amms)
}

pub async fn get_weth_values_in_amms<T, N, P>(
    amms: &[AMM],
    factories: &[Factory],
    weth: Address,
    weth_value_in_token_to_weth_pool_threshold: U256,
    step: usize,
    provider: Arc<P>,
) -> Result<Vec<U256>, AMMError>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N>,
{
    // init a new vec to hold the filtered pools
    let mut aggregate_weth_values_in_amms = vec![];

    let mut idx_from = 0;
    let mut idx_to = if step > amms.len() { amms.len() } else { step };

    for _ in (0..amms.len()).step_by(step) {
        let weth_values_in_amms = get_weth_value_in_amm_batch_request(
            &amms[idx_from..idx_to],
            factories,
            weth,
            weth_value_in_token_to_weth_pool_threshold,
            provider.clone(),
        )
        .await?;

        // add weth values in pools to the aggregate array
        aggregate_weth_values_in_amms.extend(weth_values_in_amms);

        idx_from = idx_to;

        if idx_to + step > amms.len() {
            idx_to = amms.len() - 1
        } else {
            idx_to += step;
        }
    }

    Ok(aggregate_weth_values_in_amms)
}

async fn get_weth_value_in_amm_batch_request<T, N, P>(
    amms: &[AMM],
    factories: &[Factory],
    weth: Address,
    weth_value_in_token_to_weth_pool_threshold: U256,
    provider: Arc<P>,
) -> Result<Vec<U256>, AMMError>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N>,
{
    let amms = amms.iter().map(|a| a.address()).collect::<Vec<Address>>();

    let factory_is_uni_v3 = factories
        .iter()
        .map(|d| match d {
            Factory::UniswapV2Factory(_) => false,
            Factory::UniswapV3Factory(_) => true,
        })
        .collect::<Vec<bool>>();

    let factories = factories
        .iter()
        .map(|f| f.address())
        .collect::<Vec<Address>>();

    let deployer = IGetWethValueInAMMBatchRequest::deploy_builder(
        provider,
        amms,
        factories,
        factory_is_uni_v3,
        weth,
        weth_value_in_token_to_weth_pool_threshold,
    );
    let res = deployer.call_raw().await?;

    let constructor_return = DynSolType::Array(Box::new(DynSolType::Uint(256)));
    let return_data_tokens = constructor_return.abi_decode_sequence(&res)?;

    let mut weth_value_in_pools = vec![];
    if let Some(tokens_arr) = return_data_tokens.as_array() {
        for token in tokens_arr {
            if let Some(weth_value_in_pool) = token.as_uint() {
                weth_value_in_pools.push(weth_value_in_pool.0);
            }
        }
    }

    Ok(weth_value_in_pools)
}

#[cfg(test)]
mod test {

    use alloy::{
        primitives::{address, uint},
        providers::ProviderBuilder,
        rpc::client::WsConnect,
    };
    use std::{path::Path, sync::Arc};

    use super::*;
    use crate::amm::{
        uniswap_v2::factory::UniswapV2Factory, uniswap_v3::factory::UniswapV3Factory,
    };
    use crate::sync::{checkpoint::sync_amms_from_checkpoint, sync_amms};

    const WETH_VALUE_THREASHOLD: U256 = uint!(1_000_000_000_000_000_000_U256);
    const MIN_TOKEN_PRICE_IN_WETH: U256 = uint!(0_U256);
    const WETH_ADDRESS: Address = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
    const CHECKPOINT_PATH: &str = ".temp-checkpoint.json";

    #[tokio::test]
    #[ignore] // Ignoring to not throttle the Provider on workflows
    async fn test_weth_value_filter() {
        let ipc_endpoint = std::env::var("WS").unwrap();
        let ws = WsConnect::new(ipc_endpoint.to_owned());
        let provider = Arc::new(ProviderBuilder::new().on_ws(ws).await.unwrap());

        let factories = vec![
            // Add Uniswap V2
            Factory::UniswapV2Factory(UniswapV2Factory::new(
                address!("5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f"),
                10000835,
                300,
            )),
            // Add Uniswap v3
            Factory::UniswapV3Factory(UniswapV3Factory::new(
                address!("1F98431c8aD98523631AE4a59f267346ea31F984"),
                12369621,
            )),
        ];

        let checkpoint_exists = Path::new(CHECKPOINT_PATH).exists();

        // sync all markets
        let markets = if checkpoint_exists {
            tracing::info!("Syncing pools from checkpoint");
            let (_, markets) = sync_amms_from_checkpoint(CHECKPOINT_PATH, 500, provider.clone())
                .await
                .unwrap();

            markets
        } else {
            tracing::info!("Syncing pools from inception");
            let (markets, _) = sync_amms(
                factories.clone(),
                provider.clone(),
                Some(CHECKPOINT_PATH),
                500,
            )
            .await
            .unwrap();

            markets
        };

        filter_amms_below_weth_threshold(
            markets,
            &factories,
            WETH_ADDRESS,
            WETH_VALUE_THREASHOLD,
            MIN_TOKEN_PRICE_IN_WETH,
            500,
            provider.clone(),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_usd_value_filter() {}
}
