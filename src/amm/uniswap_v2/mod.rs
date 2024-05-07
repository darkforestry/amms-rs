pub mod batch_request;
pub mod factory;

use std::sync::Arc;

use crate::{
    amm::{consts::*, AutomatedMarketMaker, IErc20},
    errors::{AMMError, ArithmeticError, EventLogError, SwapSimulationError},
};
use alloy::{
    network::Network,
    primitives::{Address, Bytes, B256, U256},
    providers::Provider,
    rpc::types::eth::Log,
    sol,
    sol_types::{SolCall, SolEvent},
    transports::Transport,
};
use async_trait::async_trait;
use num_bigfloat::BigFloat;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use self::factory::IUniswapV2Factory;

sol! {
    /// Interface of the UniswapV2Pair
    #[derive(Debug, PartialEq, Eq)]
    #[sol(rpc)]
    contract IUniswapV2Pair {
        event Sync(uint112 reserve0, uint112 reserve1);
        function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
        function token0() external view returns (address);
        function token1() external view returns (address);
        function swap(uint256 amount0Out, uint256 amount1Out, address to, bytes calldata data);
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UniswapV2Pool {
    pub address: Address,
    pub token_a: Address,
    pub token_a_decimals: u8,
    pub token_b: Address,
    pub token_b_decimals: u8,
    pub reserve_0: u128,
    pub reserve_1: u128,
    pub fee: u32,
}

#[async_trait]
impl AutomatedMarketMaker for UniswapV2Pool {
    fn address(&self) -> Address {
        self.address
    }

    #[instrument(skip(self, provider), level = "debug")]
    async fn sync<T, N, P>(&mut self, provider: Arc<P>) -> Result<(), AMMError>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        let (reserve_0, reserve_1) = self.get_reserves(provider.clone()).await?;
        tracing::info!(?reserve_0, ?reserve_1, address = ?self.address, "UniswapV2 sync");

        self.reserve_0 = reserve_0;
        self.reserve_1 = reserve_1;

        Ok(())
    }

    #[instrument(skip(self, provider), level = "debug")]
    async fn populate_data<T, N, P>(
        &mut self,
        _block_number: Option<u64>,
        provider: Arc<P>,
    ) -> Result<(), AMMError>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        batch_request::get_v2_pool_data_batch_request(self, provider.clone()).await?;

        Ok(())
    }

    fn sync_on_event_signatures(&self) -> Vec<B256> {
        vec![IUniswapV2Pair::Sync::SIGNATURE_HASH]
    }

    #[instrument(skip(self), level = "debug")]
    fn sync_from_log(&mut self, log: Log) -> Result<(), EventLogError> {
        let event_signature = log.topics()[0];

        if event_signature == IUniswapV2Pair::Sync::SIGNATURE_HASH {
            let sync_event = IUniswapV2Pair::Sync::decode_log(log.as_ref(), true)?;
            tracing::info!(reserve_0 = sync_event.reserve0, reserve_1 = sync_event.reserve1, address = ?self.address, "UniswapV2 sync event");

            self.reserve_0 = sync_event.reserve0;
            self.reserve_1 = sync_event.reserve1;

            Ok(())
        } else {
            Err(EventLogError::InvalidEventSignature)
        }
    }

    // Calculates base/quote, meaning the price of base token per quote (ie. exchange rate is X base per 1 quote)
    fn calculate_price(&self, base_token: Address) -> Result<f64, ArithmeticError> {
        Ok(q64_to_f64(self.calculate_price_64_x_64(base_token)?))
    }

    fn tokens(&self) -> Vec<Address> {
        vec![self.token_a, self.token_b]
    }

    fn simulate_swap(
        &self,
        token_in: Address,
        amount_in: U256,
    ) -> Result<U256, SwapSimulationError> {
        if self.token_a == token_in {
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
        token_in: Address,
        amount_in: U256,
    ) -> Result<U256, SwapSimulationError> {
        if self.token_a == token_in {
            let amount_out = self.get_amount_out(
                amount_in,
                U256::from(self.reserve_0),
                U256::from(self.reserve_1),
            );

            tracing::trace!(?amount_out);
            tracing::trace!(?self.reserve_0, ?self.reserve_1, "pool reserves before");

            self.reserve_0 += amount_in.to::<u128>();
            self.reserve_1 -= amount_out.to::<u128>();

            tracing::trace!(?self.reserve_0, ?self.reserve_1, "pool reserves after");

            Ok(amount_out)
        } else {
            let amount_out = self.get_amount_out(
                amount_in,
                U256::from(self.reserve_1),
                U256::from(self.reserve_0),
            );

            tracing::trace!(?amount_out);
            tracing::trace!(?self.reserve_0, ?self.reserve_1, "pool reserves before");

            self.reserve_0 -= amount_out.to::<u128>();
            self.reserve_1 += amount_in.to::<u128>();

            tracing::trace!(?self.reserve_0, ?self.reserve_1, "pool reserves after");

            Ok(amount_out)
        }
    }

    fn get_token_out(&self, token_in: Address) -> Address {
        if self.token_a == token_in {
            self.token_b
        } else {
            self.token_a
        }
    }
}

impl UniswapV2Pool {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        address: Address,
        token_a: Address,
        token_a_decimals: u8,
        token_b: Address,
        token_b_decimals: u8,
        reserve_0: u128,
        reserve_1: u128,
        fee: u32,
    ) -> UniswapV2Pool {
        UniswapV2Pool {
            address,
            token_a,
            token_a_decimals,
            token_b,
            token_b_decimals,
            reserve_0,
            reserve_1,
            fee,
        }
    }

    /// Creates a new instance of the pool from the pair address, and syncs the pool data.
    pub async fn new_from_address<T, N, P>(
        pair_address: Address,
        fee: u32,
        provider: Arc<P>,
    ) -> Result<Self, AMMError>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        let mut pool = UniswapV2Pool {
            address: pair_address,
            token_a: Address::ZERO,
            token_a_decimals: 0,
            token_b: Address::ZERO,
            token_b_decimals: 0,
            reserve_0: 0,
            reserve_1: 0,
            fee,
        };

        pool.populate_data(None, provider.clone()).await?;

        if !pool.data_is_populated() {
            return Err(AMMError::PoolDataError);
        }

        Ok(pool)
    }

    /// Creates a new instance of a the pool from a `PairCreated` event log.
    ///
    /// This method syncs the pool data.
    pub async fn new_from_log<T, N, P>(
        log: Log,
        fee: u32,
        provider: Arc<P>,
    ) -> Result<Self, AMMError>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        let event_signature = log.data().topics()[0];

        if event_signature == IUniswapV2Factory::PairCreated::SIGNATURE_HASH {
            let pair_created_event =
                factory::IUniswapV2Factory::PairCreated::decode_log(log.as_ref(), true)?;
            UniswapV2Pool::new_from_address(pair_created_event.pair, fee, provider).await
        } else {
            Err(EventLogError::InvalidEventSignature)?
        }
    }

    /// Creates a new instance of a the pool from a `PairCreated` event log.
    ///
    /// This method does not sync the pool data.
    pub fn new_empty_pool_from_log(log: Log) -> Result<Self, EventLogError> {
        let event_signature = log.topics()[0];

        if event_signature == IUniswapV2Factory::PairCreated::SIGNATURE_HASH {
            let pair_created_event =
                factory::IUniswapV2Factory::PairCreated::decode_log(log.as_ref(), true)?;

            Ok(UniswapV2Pool {
                address: pair_created_event.pair,
                token_a: pair_created_event.token0,
                token_b: pair_created_event.token1,
                token_a_decimals: 0,
                token_b_decimals: 0,
                reserve_0: 0,
                reserve_1: 0,
                fee: 0,
            })
        } else {
            Err(EventLogError::InvalidEventSignature)?
        }
    }

    /// Returns the swap fee of the pool.
    pub fn fee(&self) -> u32 {
        self.fee
    }

    /// Returns whether the pool data is populated.
    pub fn data_is_populated(&self) -> bool {
        !(self.token_a.is_zero()
            || self.token_b.is_zero()
            || self.reserve_0 == 0
            || self.reserve_1 == 0)
    }

    /// Returns the reserves of the pool.
    pub async fn get_reserves<T, N, P>(&self, provider: Arc<P>) -> Result<(u128, u128), AMMError>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        tracing::trace!("getting reserves of {}", self.address);

        // Initialize a new instance of the Pool
        let v2_pair = IUniswapV2Pair::new(self.address, provider);

        // Make a call to get the reserves
        let IUniswapV2Pair::getReservesReturn {
            reserve0: reserve_0,
            reserve1: reserve_1,
            ..
        } = match v2_pair.getReserves().call().await {
            Ok(result) => result,
            Err(contract_error) => return Err(AMMError::ContractError(contract_error)),
        };

        tracing::trace!(reserve_0, reserve_1);

        Ok((reserve_0, reserve_1))
    }

    pub async fn get_token_decimals<T, N, P>(
        &mut self,
        provider: Arc<P>,
    ) -> Result<(u8, u8), AMMError>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        let IErc20::decimalsReturn {
            _0: token_a_decimals,
        } = IErc20::new(self.token_a, provider.clone())
            .decimals()
            .call()
            .await?;

        let IErc20::decimalsReturn {
            _0: token_b_decimals,
        } = IErc20::new(self.token_b, provider)
            .decimals()
            .call()
            .await?;

        tracing::trace!(token_a_decimals, token_b_decimals);

        Ok((token_a_decimals, token_b_decimals))
    }

    pub async fn get_token_0<T, N, P>(
        &self,
        pair_address: Address,
        provider: Arc<P>,
    ) -> Result<Address, AMMError>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        let v2_pair = IUniswapV2Pair::new(pair_address, provider);

        let IUniswapV2Pair::token0Return { _0: token0 } = match v2_pair.token0().call().await {
            Ok(result) => result,
            Err(contract_error) => return Err(AMMError::ContractError(contract_error)),
        };

        Ok(token0)
    }

    pub async fn get_token_1<T, N, P>(
        &self,
        pair_address: Address,
        middleware: Arc<P>,
    ) -> Result<Address, AMMError>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        let v2_pair = IUniswapV2Pair::new(pair_address, middleware);

        let IUniswapV2Pair::token1Return { _0: token1 } = match v2_pair.token1().call().await {
            Ok(result) => result,
            Err(contract_error) => return Err(AMMError::ContractError(contract_error)),
        };

        Ok(token1)
    }

    /// Calculates the price of the base token in terms of the quote token.
    ///
    /// Returned as a Q64 fixed point number.
    pub fn calculate_price_64_x_64(&self, base_token: Address) -> Result<u128, ArithmeticError> {
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

    /// Calculates the amount received for a given `amount_in` `reserve_in` and `reserve_out`.
    pub fn get_amount_out(&self, amount_in: U256, reserve_in: U256, reserve_out: U256) -> U256 {
        tracing::trace!(?amount_in, ?reserve_in, ?reserve_out);

        if amount_in.is_zero() || reserve_in.is_zero() || reserve_out.is_zero() {
            return U256::ZERO;
        }
        let fee = (10000 - (self.fee / 10)) / 10; //Fee of 300 => (10,000 - 30) / 10  = 997
        let amount_in_with_fee = amount_in * U256::from(fee);
        let numerator = amount_in_with_fee * reserve_out;
        let denominator = reserve_in * U256::from(1000) + amount_in_with_fee;

        tracing::trace!(?fee, ?amount_in_with_fee, ?numerator, ?denominator);

        numerator / denominator
    }

    /// Returns the calldata for a swap.
    pub fn swap_calldata(
        &self,
        amount_0_out: U256,
        amount_1_out: U256,
        to: Address,
        calldata: Vec<u8>,
    ) -> Result<Bytes, alloy::dyn_abi::Error> {
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

pub fn div_uu(x: U256, y: U256) -> Result<u128, ArithmeticError> {
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
            return Err(ArithmeticError::RoundingError);
        }

        answer += xl / y;

        if answer > U256_0XFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF {
            return Ok(0_u128);
        }

        Ok(answer.to::<u128>())
    } else {
        Err(ArithmeticError::YIsZero)
    }
}

/// Converts a Q64 fixed point to a Q16 fixed point -> f64
pub fn q64_to_f64(x: u128) -> f64 {
    BigFloat::from(x)
        .div(&BigFloat::from(U128_0X10000000000000000))
        .to_f64()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use alloy::{
        primitives::{address, U256},
        providers::ProviderBuilder,
    };

    use crate::amm::AutomatedMarketMaker;

    use super::UniswapV2Pool;

    #[test]
    fn test_swap_calldata() {
        let uniswap_v2_pool = UniswapV2Pool::default();

        let _calldata = uniswap_v2_pool.swap_calldata(
            U256::from(123456789),
            U256::ZERO,
            address!("41c36f504BE664982e7519480409Caf36EE4f008"),
            vec![],
        );
    }

    #[tokio::test]
    async fn test_get_new_from_address() {
        let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT").unwrap();
        let provider = Arc::new(ProviderBuilder::new().on_http(rpc_endpoint.parse().unwrap()));

        let pool = UniswapV2Pool::new_from_address(
            address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc"),
            300,
            provider.clone(),
        )
        .await
        .unwrap();

        assert_eq!(
            pool.address,
            address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc")
        );
        assert_eq!(
            pool.token_a,
            address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")
        );
        assert_eq!(pool.token_a_decimals, 6);
        assert_eq!(
            pool.token_b,
            address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")
        );
        assert_eq!(pool.token_b_decimals, 18);
        assert_eq!(pool.fee, 300);
    }

    #[tokio::test]
    async fn test_get_pool_data() {
        let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT").unwrap();
        let provider = Arc::new(ProviderBuilder::new().on_http(rpc_endpoint.parse().unwrap()));

        let mut pool = UniswapV2Pool {
            address: address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc"),
            ..Default::default()
        };

        pool.populate_data(None, provider.clone()).await.unwrap();

        assert_eq!(
            pool.address,
            address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc")
        );
        assert_eq!(
            pool.token_a,
            address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")
        );
        assert_eq!(pool.token_a_decimals, 6);
        assert_eq!(
            pool.token_b,
            address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")
        );
        assert_eq!(pool.token_b_decimals, 18);
    }

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

        assert!(x.calculate_price(token_a).unwrap() != 0.0);
        assert!(x.calculate_price(token_b).unwrap() != 0.0);
    }

    #[tokio::test]
    async fn test_calculate_price() {
        let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT").unwrap();
        let provider = Arc::new(ProviderBuilder::new().on_http(rpc_endpoint.parse().unwrap()));

        let mut pool = UniswapV2Pool {
            address: address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc"),
            ..Default::default()
        };

        pool.populate_data(None, provider.clone()).await.unwrap();

        pool.reserve_0 = 47092140895915;
        pool.reserve_1 = 28396598565590008529300;

        let price_a_64_x = pool.calculate_price(pool.token_a).unwrap();
        let price_b_64_x = pool.calculate_price(pool.token_b).unwrap();

        // No precision loss: 30591574867092394336528 / 2**64
        assert_eq!(1658.3725965327264, price_b_64_x);
        // Precision loss: 11123401407064628 / 2**64
        assert_eq!(0.0006030007985483893, price_a_64_x);
    }

    #[tokio::test]
    async fn test_calculate_price_64_x_64() {
        let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT").unwrap();
        let provider = Arc::new(ProviderBuilder::new().on_http(rpc_endpoint.parse().unwrap()));

        let mut pool = UniswapV2Pool {
            address: address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc"),
            ..Default::default()
        };

        pool.populate_data(None, provider.clone()).await.unwrap();

        pool.reserve_0 = 47092140895915;
        pool.reserve_1 = 28396598565590008529300;

        let price_a_64_x = pool.calculate_price_64_x_64(pool.token_a).unwrap();
        let price_b_64_x = pool.calculate_price_64_x_64(pool.token_b).unwrap();

        assert_eq!(30591574867092394336528, price_b_64_x);
        assert_eq!(11123401407064628, price_a_64_x);
    }
}
