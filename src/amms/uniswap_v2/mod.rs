use super::{
    amm::{AutomatedMarketMaker, AMM},
    consts::{F64_FEE_ONE, U256_FEE_ONE, U32_FEE_ONE},
    error::AMMError,
    factory::{AutomatedMarketMakerFactory, DiscoverySync},
    Token,
};

use alloy::{
    eips::BlockId,
    network::Network,
    primitives::{Address, Bytes, B256, U256},
    providers::Provider,
    rpc::types::Log,
    sol,
    sol_types::{SolCall, SolEvent, SolValue},
    transports::Transport,
};
use futures::{stream::FuturesUnordered, StreamExt};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, future::Future, hash::Hash, sync::Arc};
use thiserror::Error;
use tracing::info;
use IGetUniswapV2PoolDataBatchRequest::IGetUniswapV2PoolDataBatchRequestInstance;
use IUniswapV2Factory::IUniswapV2FactoryInstance;

sol!(
// UniswapV2Factory
#[allow(missing_docs)]
#[derive(Debug)]
#[sol(rpc)]
contract IUniswapV2Factory {
    event PairCreated(address indexed token0, address indexed token1, address pair, uint256);
    function allPairs(uint256) external view returns (address pair);
    function allPairsLength() external view returns (uint256);

}

#[derive(Debug, PartialEq, Eq)]
#[sol(rpc)]
contract IUniswapV2Pair {
    event Sync(uint112 reserve0, uint112 reserve1);
    function token0() external view returns (address);
    function token1() external view returns (address);
    function swap(uint256 amount0Out, uint256 amount1Out, address to, bytes calldata data);
    function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
});

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    IGetUniswapV2PairsBatchRequest,
    "contracts/out/GetUniswapV2PairsBatchRequest.sol/GetUniswapV2PairsBatchRequest.json"
);

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    IGetUniswapV2PoolDataBatchRequest,
    "contracts/out/GetUniswapV2PoolDataBatchRequest.sol/GetUniswapV2PoolDataBatchRequest.json"
);

#[derive(Error, Debug)]
pub enum UniswapV2Error {}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UniswapV2Pool {
    pub address: Address,
    pub token_a: Token,
    pub token_b: Token,
    pub reserve_0: u128,
    pub reserve_1: u128,
    pub fee: u32,
}

impl AutomatedMarketMaker for UniswapV2Pool {
    fn address(&self) -> Address {
        self.address
    }

    fn sync_events(&self) -> Vec<B256> {
        vec![IUniswapV2Pair::Sync::SIGNATURE_HASH]
    }

    fn sync(&mut self, log: &Log) -> Result<(), AMMError> {
        let sync_event = IUniswapV2Pair::Sync::decode_log(&log.inner, false)?;

        let (reserve_0, reserve_1) = (
            sync_event.reserve0.to::<u128>(),
            sync_event.reserve1.to::<u128>(),
        );

        info!(
            target = "amm::uniswap_v2::sync",
            address = ?self.address,
            reserve_0, reserve_1, "Sync"
        );

        self.reserve_0 = reserve_0;
        self.reserve_1 = reserve_1;
        Ok(())
    }

    fn simulate_swap(
        &self,
        base_token: Address,
        _quote_token: Address,
        amount_in: U256,
    ) -> Result<U256, AMMError> {
        if self.token_a.address == base_token {
            Ok(self.get_amount_out(
                amount_in,
                U256::from(self.reserve_0),
                U256::from(self.reserve_1),
            ))
        } else {
            Ok(self.get_amount_out(
                amount_in,
                U256::from(self.reserve_1),
                U256::from(self.reserve_0),
            ))
        }
    }

    fn simulate_swap_mut(
        &mut self,
        base_token: Address,
        _quote_token: Address,
        amount_in: U256,
    ) -> Result<U256, AMMError> {
        if self.token_a.address == base_token {
            let amount_out = self.get_amount_out(
                amount_in,
                U256::from(self.reserve_0),
                U256::from(self.reserve_1),
            );

            self.reserve_0 += amount_in.to::<u128>();
            self.reserve_1 -= amount_out.to::<u128>();

            Ok(amount_out)
        } else {
            let amount_out = self.get_amount_out(
                amount_in,
                U256::from(self.reserve_1),
                U256::from(self.reserve_0),
            );

            self.reserve_0 -= amount_out.to::<u128>();
            self.reserve_1 += amount_in.to::<u128>();

            Ok(amount_out)
        }
    }

    fn tokens(&self) -> Vec<Address> {
        vec![self.token_a.address, self.token_b.address]
    }

    fn calculate_price(&self, base_token: Address, _quote_token: Address) -> Result<f64, AMMError> {
        // Decimals are intentionally swapped as we are multiplying rather than dividing
        let (r_0, r_1) = (
            self.reserve_0 as f64 * (10f64).powi(self.token_b.decimals as i32),
            self.reserve_1 as f64 * (10f64).powi(self.token_a.decimals as i32),
        );
        let (reserve_in, reserve_out) = if base_token == self.token_a {
            Ok((r_0, r_1))
        } else if base_token == self.token_b {
            Ok((r_1, r_0))
        } else {
            Err(AMMError::IncompatibleToken)
        }?;
        let numerator = reserve_out * F64_FEE_ONE;
        let denominator = reserve_in * (U32_FEE_ONE - self.fee) as f64;
        Ok(numerator / denominator)
    }

    async fn init<T, N, P>(
        mut self,
        block_number: BlockId,
        provider: Arc<P>,
    ) -> Result<Self, AMMError>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        let deployer = IGetUniswapV2PoolDataBatchRequestInstance::deploy_builder(
            provider.clone(),
            vec![self.address()],
        );

        let res = deployer.call_raw().block(block_number).await?;

        let pool_data =
            <Vec<(Address, Address, u128, u128, u32, u32)> as SolValue>::abi_decode(&res, false)?
                [0];

        if pool_data.0.is_zero() {
            todo!("Return error");
        }

        self.token_a = Token::new_with_decimals(pool_data.0, pool_data.4 as u8);
        self.token_b = Token::new_with_decimals(pool_data.1, pool_data.5 as u8);
        self.reserve_0 = pool_data.2;
        self.reserve_1 = pool_data.3;

        // TODO: populate fee?

        Ok(self)
    }
}

impl UniswapV2Pool {
    // Create a new, unsynced UniswapV2 pool
    // TODO: update the init function to derive the fee
    pub fn new(address: Address, fee: u32) -> Self {
        Self {
            address,
            fee,
            ..Default::default()
        }
    }

    /// Calculates the amount received for a given `amount_in` `reserve_in` and `reserve_out`.
    pub fn get_amount_out(&self, amount_in: U256, reserve_in: U256, reserve_out: U256) -> U256 {
        if amount_in.is_zero() || reserve_in.is_zero() || reserve_out.is_zero() {
            return U256::ZERO;
        }

        // TODO: we could set this as the fee on the pool instead of calculating this
        let fee = U32_FEE_ONE - self.fee;
        let amount_in_with_fee = amount_in * U256::from(fee);
        let numerator = amount_in_with_fee * reserve_out;
        let denominator = reserve_in * U256_FEE_ONE + amount_in_with_fee;

        numerator / denominator
    }

    pub fn swap_calldata(
        &self,
        amount_0_out: U256,
        amount_1_out: U256,
        to: Address,
        calldata: Vec<u8>,
    ) -> Result<Bytes, AMMError> {
        Ok(IUniswapV2Pair::swapCall {
            amount0Out: amount_0_out,
            amount1Out: amount_1_out,
            to,
            data: calldata.into(),
        }
        .abi_encode()
        .into())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct UniswapV2Factory {
    pub address: Address,
    pub fee: u32,
    pub creation_block: u64,
}

impl UniswapV2Factory {
    pub fn new(address: Address, fee: u32, creation_block: u64) -> Self {
        Self {
            address,
            creation_block,
            fee,
        }
    }

    pub async fn get_all_pairs<T, N, P>(
        factory_address: Address,
        block_number: BlockId,
        provider: Arc<P>,
    ) -> Result<Vec<Address>, AMMError>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        let factory = IUniswapV2FactoryInstance::new(factory_address, provider.clone());
        let pairs_length = factory
            .allPairsLength()
            .call()
            .block(block_number)
            .await?
            ._0
            .to::<usize>();

        let step = 766;
        let mut futures_unordered = FuturesUnordered::new();
        for i in (0..pairs_length).step_by(step) {
            // Note that the batch contract handles if the step is greater than the pairs length
            // So we can pass the step in as is without checking for this condition
            let deployer = IGetUniswapV2PairsBatchRequest::deploy_builder(
                provider.clone(),
                U256::from(i),
                U256::from(step),
                factory_address,
            );

            futures_unordered.push(async move {
                let res = deployer.call_raw().block(block_number).await?;
                let return_data = <Vec<Address> as SolValue>::abi_decode(&res, false)?;

                Ok::<Vec<Address>, AMMError>(return_data)
            });
        }

        let mut pairs = Vec::new();
        while let Some(res) = futures_unordered.next().await {
            let tokens = res?;
            for token in tokens {
                if !token.is_zero() {
                    pairs.push(token);
                }
            }
        }

        Ok(pairs)
    }

    pub async fn sync_all_pools<T, N, P>(
        amms: Vec<AMM>,
        block_number: BlockId,
        provider: Arc<P>,
    ) -> Result<Vec<AMM>, AMMError>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        let step = 120;
        let pairs = amms
            .iter()
            .chunks(step)
            .into_iter()
            .map(|chunk| chunk.map(|amm| amm.address()).collect())
            .collect::<Vec<Vec<Address>>>();

        let mut futures_unordered = FuturesUnordered::new();
        for group in pairs {
            let deployer = IGetUniswapV2PoolDataBatchRequestInstance::deploy_builder(
                provider.clone(),
                group.clone(),
            );

            futures_unordered.push(async move {
                let res = deployer.call_raw().block(block_number).await?;

                let return_data =
                    <Vec<(Address, Address, u128, u128, u32, u32)> as SolValue>::abi_decode(
                        &res, false,
                    )?;

                Ok::<(Vec<Address>, Vec<(Address, Address, u128, u128, u32, u32)>), AMMError>((
                    group,
                    return_data,
                ))
            });
        }

        let mut amms = amms
            .into_iter()
            .map(|amm| (amm.address(), amm))
            .collect::<HashMap<_, _>>();

        while let Some(res) = futures_unordered.next().await {
            let (group, return_data) = res?;
            for (pool_data, pool_address) in return_data.iter().zip(group.iter()) {
                // If the pool token A is not zero, signaling that the pool data was polulated

                if pool_data.0.is_zero() {
                    continue;
                }

                let amm = amms.get_mut(pool_address).unwrap();

                let AMM::UniswapV2Pool(pool) = amm else {
                    // NOTE: We should never receive a non UniswapV2Pool AMM here, we can handle this more gracefully in the future
                    panic!("Unexpected pool type")
                };

                pool.token_a = Token::new_with_decimals(pool_data.0, pool_data.4 as u8);
                pool.token_b = Token::new_with_decimals(pool_data.1, pool_data.5 as u8);
                pool.reserve_0 = pool_data.2;
                pool.reserve_1 = pool_data.3;
            }
        }

        let amms = amms
            .into_iter()
            .filter_map(|(_, amm)| {
                if amm.tokens().iter().any(|t| t.is_zero()) {
                    None
                } else {
                    Some(amm)
                }
            })
            .collect();

        Ok(amms)
    }
}

impl AutomatedMarketMakerFactory for UniswapV2Factory {
    type PoolVariant = UniswapV2Pool;

    fn address(&self) -> Address {
        self.address
    }

    fn pool_creation_event(&self) -> B256 {
        IUniswapV2Factory::PairCreated::SIGNATURE_HASH
    }

    fn create_pool(&self, log: Log) -> Result<AMM, AMMError> {
        let event = IUniswapV2Factory::PairCreated::decode_log(&log.inner, false)?;
        Ok(AMM::UniswapV2Pool(UniswapV2Pool {
            address: event.pair,
            token_a: event.token0.into(),
            token_b: event.token1.into(),
            reserve_0: 0,
            reserve_1: 0,
            fee: self.fee,
        }))
    }

    fn creation_block(&self) -> u64 {
        self.creation_block
    }
}

impl DiscoverySync for UniswapV2Factory {
    fn discover<T, N, P>(
        &self,
        to_block: BlockId,
        provider: Arc<P>,
    ) -> impl Future<Output = Result<Vec<AMM>, AMMError>>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        info!(
            target = "amms::uniswap_v2::discover",
            address = ?self.address,
            "Discovering all pools"
        );

        let provider = provider.clone();
        async move {
            let pairs =
                UniswapV2Factory::get_all_pairs(self.address, to_block, provider.clone()).await?;

            Ok(pairs
                .into_iter()
                .map(|pair| {
                    AMM::UniswapV2Pool(UniswapV2Pool {
                        address: pair,
                        token_a: Address::default().into(),
                        token_b: Address::default().into(),
                        reserve_0: 0,
                        reserve_1: 0,
                        fee: self.fee,
                    })
                })
                .collect())
        }
    }

    fn sync<T, N, P>(
        &self,
        amms: Vec<AMM>,
        to_block: BlockId,
        provider: Arc<P>,
    ) -> impl Future<Output = Result<Vec<AMM>, AMMError>>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        info!(
            target = "amms::uniswap_v2::sync",
            address = ?self.address,
            "Syncing all pools"
        );

        UniswapV2Factory::sync_all_pools(amms, to_block, provider)
    }
}

#[cfg(test)]
mod tests {
    use crate::amms::{
        amm::AutomatedMarketMaker, error::AMMError, uniswap_v2::UniswapV2Pool, Token,
    };
    use alloy::primitives::{address, Address};
    use float_cmp::assert_approx_eq;

    fn get_test_pool(reserve_0: u128, reserve_1: u128) -> UniswapV2Pool {
        UniswapV2Pool {
            address: address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc"),
            token_a: Token::new_with_decimals(
                address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
                6,
            ),
            token_b: Token::new_with_decimals(
                address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
                18,
            ),
            reserve_0: reserve_0,
            reserve_1: reserve_1,
            fee: 3000,
        }
    }

    #[test]
    fn test_calculate_price_edge_case() {
        let pool = get_test_pool(23595096345912178729927, 154664232014390554564);
        assert_ne!(
            pool.calculate_price(pool.token_a.address, Address::default())
                .unwrap(),
            0.0
        );
        assert_ne!(
            pool.calculate_price(pool.token_b.address, Address::default())
                .unwrap(),
            0.0
        );
    }

    #[test]
    fn test_calculate_price() {
        let pool = get_test_pool(47092140895915, 28396598565590008529300);

        let price_a_for_b = pool
            .calculate_price(pool.token_a.address, Address::default())
            .unwrap();
        let price_b_for_a = pool
            .calculate_price(pool.token_b.address, Address::default())
            .unwrap();

        // FWIW, the representation is accurate to 0 and 1 ULPs on this example, but we don't want a change detector
        assert_approx_eq!(f64, 1663.362684586485983229152871, price_b_for_a, ulps = 4);
        assert_approx_eq!(
            f64,
            0.0006048152442812330502786409979,
            price_a_for_b,
            ulps = 4
        );
    }

    #[test]
    fn test_incompatible_token() {
        let pool = get_test_pool(47092140895915, 28396598565590008529300);

        let invalid_price = pool.calculate_price(Address::default(), Address::default());

        assert!(matches!(invalid_price, Err(AMMError::IncompatibleToken)));
    }

    #[test]
    fn test_zero_reserve() {
        let pool = get_test_pool(0, 28396598565590008529300);

        let infinite_price = pool.calculate_price(pool.token_a.address, Address::default());
        let zero_price = pool.calculate_price(pool.token_b.address, Address::default());

        assert_eq!(infinite_price.unwrap(), f64::INFINITY);
        assert_eq!(zero_price.unwrap(), 0.0);
    }
}
