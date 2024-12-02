pub mod error;

use super::{
    amm::{AutomatedMarketMaker, AMM},
    consts::{
        MPFR_T_PRECISION, U128_0X10000000000000000, U256_0X100, U256_0X10000, U256_0X100000000,
        U256_0XFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF,
        U256_0XFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF, U256_1, U256_1000, U256_128,
        U256_16, U256_191, U256_192, U256_2, U256_255, U256_32, U256_4, U256_64, U256_8,
    },
    error::AMMError,
    factory::{AutomatedMarketMakerFactory, DiscoverySync, Factory},
};

use alloy::{
    network::Network,
    primitives::{Address, Bytes, B256, U256},
    providers::Provider,
    rpc::types::Log,
    sol,
    sol_types::{SolCall, SolEvent, SolValue},
    transports::Transport,
};
use error::UniswapV2Error;
use eyre::Result;
use futures::{stream::FuturesUnordered, StreamExt};
use itertools::Itertools;
use rug::Float;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, future::Future, hash::Hash, sync::Arc};
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UniswapV2Pool {
    pub address: Address,
    pub token_a: Address,
    pub token_a_decimals: u8,
    pub token_b: Address,
    pub token_b_decimals: u8,
    pub reserve_0: u128,
    pub reserve_1: u128,
    pub fee: usize,
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
        // tracing::info!(reserve_0, reserve_1, address = ?self.address, "UniswapV2 sync event");

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
        if self.token_a == base_token {
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
        if self.token_a == base_token {
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
        vec![self.token_a, self.token_b]
    }

    fn calculate_price(&self, base_token: Address, _quote_token: Address) -> Result<f64, AMMError> {
        let price = self.calculate_price_64_x_64(base_token)?;
        q64_to_float(price)
    }
}

pub fn q64_to_float(num: u128) -> Result<f64, AMMError> {
    let float_num = u128_to_float(num)?;
    let divisor = u128_to_float(U128_0X10000000000000000)?;
    Ok((float_num / divisor).to_f64())
}

pub fn u128_to_float(num: u128) -> Result<Float, AMMError> {
    let value_string = num.to_string();
    let parsed_value =
        Float::parse_radix(value_string, 10)?;
    Ok(Float::with_val(MPFR_T_PRECISION, parsed_value))
}

impl UniswapV2Pool {
    /// Calculates the amount received for a given `amount_in` `reserve_in` and `reserve_out`.
    pub fn get_amount_out(&self, amount_in: U256, reserve_in: U256, reserve_out: U256) -> U256 {
        if amount_in.is_zero() || reserve_in.is_zero() || reserve_out.is_zero() {
            return U256::ZERO;
        }

        // TODO: we could set this as the fee on the pool instead of calculating this
        let fee = (10000 - (self.fee / 10)) / 10; // Fee of 300 => (10,000 - 30) / 10  = 997
        let amount_in_with_fee = amount_in * U256::from(fee);
        let numerator = amount_in_with_fee * reserve_out;
        let denominator = reserve_in * U256_1000 + amount_in_with_fee;

        numerator / denominator
    }

    /// Calculates the price of the base token in terms of the quote token.
    ///
    /// Returned as a Q64 fixed point number.
    pub fn calculate_price_64_x_64(&self, base_token: Address) -> Result<u128, AMMError> {
        let decimal_shift = self.token_a_decimals as i8 - self.token_b_decimals as i8;

        let (r_0, r_1) = if decimal_shift < 0 {
            (
                U256::from(self.reserve_0)
                    * U256::from(10u128.pow(decimal_shift.unsigned_abs() as u32)),
                U256::from(self.reserve_1),
            )
        } else {
            (
                U256::from(self.reserve_0),
                U256::from(self.reserve_1) * U256::from(10u128.pow(decimal_shift as u32)),
            )
        };

        if base_token == self.token_a {
            if r_0.is_zero() {
                Ok(U128_0X10000000000000000)
            } else {
                div_uu(r_1, r_0)
            }
        } else if r_1.is_zero() {
            Ok(U128_0X10000000000000000)
        } else {
            div_uu(r_0, r_1)
        }
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

pub fn div_uu(x: U256, y: U256) -> Result<u128, AMMError> {
    if !y.is_zero() {
        let mut answer;

        if x <= U256_0XFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF {
            answer = (x << U256_64) / y;
        } else {
            let mut msb = U256_192;
            let mut xc = x >> U256_192;

            if xc >= U256_0X100000000 {
                xc >>= U256_32;
                msb += U256_32;
            }

            if xc >= U256_0X10000 {
                xc >>= U256_16;
                msb += U256_16;
            }

            if xc >= U256_0X100 {
                xc >>= U256_8;
                msb += U256_8;
            }

            if xc >= U256_16 {
                xc >>= U256_4;
                msb += U256_4;
            }

            if xc >= U256_4 {
                xc >>= U256_2;
                msb += U256_2;
            }

            if xc >= U256_2 {
                msb += U256_1;
            }

            answer = (x << (U256_255 - msb)) / (((y - U256_1) >> (msb - U256_191)) + U256_1);
        }

        if answer > U256_0XFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF {
            return Ok(0);
        }

        let hi = answer * (y >> U256_128);
        let mut lo = answer * (y & U256_0XFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF);

        let mut xh = x >> U256_192;
        let mut xl = x << U256_64;

        if xl < lo {
            xh -= U256_1;
        }

        xl = xl.overflowing_sub(lo).0;
        lo = hi << U256_128;

        if xl < lo {
            xh -= U256_1;
        }

        xl = xl.overflowing_sub(lo).0;

        if xh != hi >> U256_128 {
            return Err(UniswapV2Error::RoundingError.into());
        }

        answer += xl / y;

        if answer > U256_0XFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF {
            return Ok(0_u128);
        }

        Ok(answer.to::<u128>())
    } else {
        Err(UniswapV2Error::DivisionByZero.into())
    }
}

impl From<UniswapV2Pool> for AMM {
    fn from(val: UniswapV2Pool) -> Self {
        AMM::UniswapV2Pool(val)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct UniswapV2Factory {
    pub address: Address,
    pub fee: usize,
    pub creation_block: u64,
}

impl UniswapV2Factory {
    pub fn new(address: Address, fee: usize, creation_block: u64) -> Self {
        Self {
            address,
            creation_block,
            fee,
        }
    }

    async fn get_all_pairs<T, N, P>(
        factory_address: Address,
        block_number: u64,
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
            .block(block_number.into())
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
                let res = deployer.call_raw().block(block_number.into()).await?;
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

    async fn sync_all_pools<T, N, P>(
        amms: Vec<AMM>,
        block_number: u64,
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
                let res = deployer.call_raw().block(block_number.into()).await?;

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

                pool.token_a = pool_data.0;
                pool.token_b = pool_data.1;
                pool.reserve_0 = pool_data.2;
                pool.reserve_1 = pool_data.3;
                pool.token_a_decimals = pool_data.4 as u8;
                pool.token_b_decimals = pool_data.5 as u8;
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

impl From<UniswapV2Factory> for Factory {
    fn from(val: UniswapV2Factory) -> Self {
        Factory::UniswapV2Factory(val)
    }
}

impl AutomatedMarketMakerFactory for UniswapV2Factory {
    type PoolVariant = UniswapV2Pool;

    fn address(&self) -> Address {
        self.address
    }

    fn discovery_event(&self) -> B256 {
        IUniswapV2Factory::PairCreated::SIGNATURE_HASH
    }

    fn create_pool(&self, log: Log) -> Result<AMM, AMMError> {
        let event = IUniswapV2Factory::PairCreated::decode_log(&log.inner, false)?;
        Ok(AMM::UniswapV2Pool(UniswapV2Pool {
            address: event.pair,
            token_a: event.token0,
            token_a_decimals: 0,
            token_b: event.token1,
            token_b_decimals: 0,
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
        to_block: u64,
        provider: Arc<P>,
    ) -> impl Future<Output = Result<Vec<AMM>, AMMError>>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        let provider = provider.clone();
        async move {
            let pairs =
                UniswapV2Factory::get_all_pairs(self.address, to_block, provider.clone()).await?;

            Ok(pairs
                .into_iter()
                .map(|pair| {
                    AMM::UniswapV2Pool(UniswapV2Pool {
                        address: pair,
                        token_a: Address::default(),
                        token_a_decimals: 0,
                        token_b: Address::default(),
                        token_b_decimals: 0,
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
        to_block: u64,
        provider: Arc<P>,
    ) -> impl Future<Output = Result<Vec<AMM>, AMMError>>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        UniswapV2Factory::sync_all_pools(amms, to_block, provider)
    }
}

#[cfg(test)]
mod tests {
    use crate::amms::{amm::AutomatedMarketMaker, uniswap_v2::UniswapV2Pool};
    use alloy::primitives::{address, Address};

    #[test]
    fn test_calculate_price_edge_case() {
        let token_a = address!("0d500b1d8e8ef31e21c99d1db9a6444d3adf1270");
        let token_b = address!("8f18dc399594b451eda8c5da02d0563c0b2d0f16");
        let x = UniswapV2Pool {
            address: address!("652a7b75c229850714d4a11e856052aac3e9b065"),
            token_a,
            token_a_decimals: 18,
            token_b,
            token_b_decimals: 9,
            reserve_0: 23595096345912178729927,
            reserve_1: 154664232014390554564,
            fee: 300,
        };

        assert!(x.calculate_price(token_a, Address::default()).unwrap() != 0.0);
        assert!(x.calculate_price(token_b, Address::default()).unwrap() != 0.0);
    }

    #[tokio::test]
    async fn test_calculate_price() {
        let pool = UniswapV2Pool {
            address: address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc"),
            token_a: address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
            token_b: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
            token_a_decimals: 6,
            token_b_decimals: 18,
            reserve_0: 47092140895915,
            reserve_1: 28396598565590008529300,
            fee: 300,
        };

        let price_a_64_x = pool
            .calculate_price(pool.token_a, Address::default())
            .unwrap();
        let price_b_64_x = pool
            .calculate_price(pool.token_b, Address::default())
            .unwrap();

        // No precision loss: 30591574867092394336528 / 2**64
        assert_eq!(1658.3725965327264, price_b_64_x);
        // Precision loss: 11123401407064628 / 2**64
        assert_eq!(0.0006030007985483893, price_a_64_x);
    }

    #[tokio::test]
    async fn test_calculate_price_64_x_64() {
        let pool = UniswapV2Pool {
            address: address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc"),
            token_a: address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
            token_b: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
            token_a_decimals: 6,
            token_b_decimals: 18,
            reserve_0: 47092140895915,
            reserve_1: 28396598565590008529300,
            fee: 300,
        };

        let price_a_64_x = pool.calculate_price_64_x_64(pool.token_a).unwrap();
        let price_b_64_x = pool.calculate_price_64_x_64(pool.token_b).unwrap();

        assert_eq!(30591574867092394336528, price_b_64_x);
        assert_eq!(11123401407064628, price_a_64_x);
    }
}
