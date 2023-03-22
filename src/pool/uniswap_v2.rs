use std::sync::Arc;

use ethers::{
    abi::{ethabi::Bytes, ParamType, Token},
    providers::Middleware,
    types::{Log, H160, H256, U256},
};

use crate::{
    abi, batch_requests,
    errors::{ArithmeticError, CFMMError},
};

use super::fixed_point_math::{self};

pub const SYNC_EVENT_SIGNATURE: H256 = H256([
    28, 65, 30, 154, 150, 224, 113, 36, 28, 47, 33, 247, 114, 107, 23, 174, 137, 227, 202, 180,
    199, 139, 229, 14, 6, 43, 3, 169, 255, 251, 186, 209,
]);

#[derive(Debug, Clone, Copy, Default)]
pub struct UniswapV2Pool {
    pub address: H160,
    pub token_a: H160,
    pub token_a_decimals: u8,
    pub token_b: H160,
    pub token_b_decimals: u8,
    pub reserve_0: u128,
    pub reserve_1: u128,
    pub fee: u32,
}

impl UniswapV2Pool {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        address: H160,
        token_a: H160,
        token_a_decimals: u8,
        token_b: H160,
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

    //Creates a new instance of the pool from the pair address, and syncs the pool data
    pub async fn new_from_address<M: Middleware>(
        pair_address: H160,
        middleware: Arc<M>,
    ) -> Result<Self, CFMMError<M>> {
        let mut pool = UniswapV2Pool {
            address: pair_address,
            token_a: H160::zero(),
            token_a_decimals: 0,
            token_b: H160::zero(),
            token_b_decimals: 0,
            reserve_0: 0,
            reserve_1: 0,
            fee: 300,
        };

        pool.get_pool_data(middleware.clone()).await?;

        if !pool.data_is_populated() {
            return Err(CFMMError::PoolDataError);
        }

        Ok(pool)
    }
    pub async fn new_from_event_log<M: Middleware>(
        log: Log,
        middleware: Arc<M>,
    ) -> Result<Self, CFMMError<M>> {
        let tokens = ethers::abi::decode(&[ParamType::Address, ParamType::Uint(256)], &log.data)?;
        let pair_address = tokens[0].to_owned().into_address().unwrap();
        UniswapV2Pool::new_from_address(pair_address, middleware).await
    }

    pub fn new_empty_pool_from_event_log<M: Middleware>(log: Log) -> Result<Self, CFMMError<M>> {
        let tokens = ethers::abi::decode(&[ParamType::Address, ParamType::Uint(256)], &log.data)?;
        let token_a = H160::from(log.topics[0]);
        let token_b = H160::from(log.topics[1]);
        let address = tokens[0].to_owned().into_address().unwrap();

        Ok(UniswapV2Pool {
            address,
            token_a,
            token_b,
            token_a_decimals: 0,
            token_b_decimals: 0,
            reserve_0: 0,
            reserve_1: 0,
            fee: 300,
        })
    }

    pub fn fee(&self) -> u32 {
        self.fee
    }

    pub async fn get_pool_data<M: Middleware>(
        &mut self,
        middleware: Arc<M>,
    ) -> Result<(), CFMMError<M>> {
        batch_requests::uniswap_v2::get_v2_pool_data_batch_request(self, middleware.clone())
            .await?;

        Ok(())
    }

    pub fn data_is_populated(&self) -> bool {
        !(self.token_a.is_zero()
            || self.token_b.is_zero()
            || self.reserve_0 == 0
            || self.reserve_1 == 0)
    }

    pub async fn get_reserves<M: Middleware>(
        &self,
        middleware: Arc<M>,
    ) -> Result<(u128, u128), CFMMError<M>> {
        //Initialize a new instance of the Pool
        let v2_pair = abi::IUniswapV2Pair::new(self.address, middleware);
        // Make a call to get the reserves
        let (reserve_0, reserve_1, _) = match v2_pair.get_reserves().call().await {
            Ok(result) => result,
            Err(contract_error) => return Err(CFMMError::ContractError(contract_error)),
        };

        Ok((reserve_0, reserve_1))
    }

    pub async fn sync_pool<M: Middleware>(
        &mut self,
        middleware: Arc<M>,
    ) -> Result<(), CFMMError<M>> {
        (self.reserve_0, self.reserve_1) = self.get_reserves(middleware).await?;

        Ok(())
    }

    pub async fn get_token_decimals<M: Middleware>(
        &mut self,
        middleware: Arc<M>,
    ) -> Result<(u8, u8), CFMMError<M>> {
        let token_a_decimals = abi::IErc20::new(self.token_a, middleware.clone())
            .decimals()
            .call()
            .await?;

        let token_b_decimals = abi::IErc20::new(self.token_b, middleware)
            .decimals()
            .call()
            .await?;

        Ok((token_a_decimals, token_b_decimals))
    }

    pub async fn get_token_0<M: Middleware>(
        &self,
        pair_address: H160,
        middleware: Arc<M>,
    ) -> Result<H160, CFMMError<M>> {
        let v2_pair = abi::IUniswapV2Pair::new(pair_address, middleware);

        let token0 = match v2_pair.token_0().call().await {
            Ok(result) => result,
            Err(contract_error) => return Err(CFMMError::ContractError(contract_error)),
        };

        Ok(token0)
    }

    pub async fn get_token_1<M: Middleware>(
        &self,
        pair_address: H160,
        middleware: Arc<M>,
    ) -> Result<H160, CFMMError<M>> {
        let v2_pair = abi::IUniswapV2Pair::new(pair_address, middleware);

        let token1 = match v2_pair.token_1().call().await {
            Ok(result) => result,
            Err(contract_error) => return Err(CFMMError::ContractError(contract_error)),
        };

        Ok(token1)
    }

    //Calculates base/quote, meaning the price of base token per quote (ie. exchange rate is X base per 1 quote)
    pub fn calculate_price(&self, base_token: H160) -> Result<f64, ArithmeticError> {
        Ok(fixed_point_math::q64_to_f64(
            self.calculate_price_64_x_64(base_token)?,
        ))
    }

    pub fn calculate_price_64_x_64(&self, base_token: H160) -> Result<u128, ArithmeticError> {
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
            Ok(fixed_point_math::div_uu(r_1, r_0)?)
        } else {
            Ok(fixed_point_math::div_uu(r_0, r_1))?
        }
    }

    pub fn address(&self) -> H160 {
        self.address
    }

    pub fn update_pool_from_sync_log(&mut self, sync_log: &Log) {
        (self.reserve_0, self.reserve_1) = self.decode_sync_log(sync_log);
    }

    //Returns reserve0, reserve1
    pub fn decode_sync_log(&self, sync_log: &Log) -> (u128, u128) {
        let data = ethers::abi::decode(
            &[
                ParamType::Uint(128), //reserve0
                ParamType::Uint(128),
            ],
            &sync_log.data,
        )
        .expect("Could not get log data");

        (
            data[0]
                .to_owned()
                .into_uint()
                .expect("Could not convert reserve0 in to uint")
                .as_u128(),
            data[1]
                .to_owned()
                .into_uint()
                .expect("Could not convert reserve1 in to uint")
                .as_u128(),
        )
    }

    pub fn simulate_swap(&self, token_in: H160, amount_in: U256) -> U256 {
        if self.token_a == token_in {
            self.get_amount_out(
                amount_in,
                U256::from(self.reserve_0),
                U256::from(self.reserve_1),
            )
        } else {
            self.get_amount_out(
                amount_in,
                U256::from(self.reserve_1),
                U256::from(self.reserve_0),
            )
        }
    }

    pub fn simulate_swap_mut(&mut self, token_in: H160, amount_in: U256) -> U256 {
        if self.token_a == token_in {
            let amount_out = self.get_amount_out(
                amount_in,
                U256::from(self.reserve_0),
                U256::from(self.reserve_1),
            );

            self.reserve_0 += amount_in.as_u128();
            self.reserve_1 -= amount_out.as_u128();

            amount_out
        } else {
            let amount_out = self.get_amount_out(
                amount_in,
                U256::from(self.reserve_1),
                U256::from(self.reserve_0),
            );

            self.reserve_0 -= amount_out.as_u128();
            self.reserve_1 += amount_in.as_u128();

            amount_out
        }
    }

    pub fn get_amount_out(&self, amount_in: U256, reserve_in: U256, reserve_out: U256) -> U256 {
        if amount_in.is_zero() || reserve_in.is_zero() || reserve_out.is_zero() {
            return U256::zero();
        }

        let amount_in_with_fee = amount_in * U256::from(997);
        let numerator = amount_in_with_fee * reserve_out;
        let denominator = reserve_in * U256::from(1000) + amount_in_with_fee;

        numerator / denominator
    }

    pub fn swap_calldata(
        &self,
        amount_0_out: U256,
        amount_1_out: U256,
        to: H160,
        calldata: Vec<u8>,
    ) -> Bytes {
        let input_tokens = vec![
            Token::Uint(amount_0_out),
            Token::Uint(amount_1_out),
            Token::Address(to),
            Token::Bytes(calldata),
        ];

        abi::IUNISWAPV2PAIR_ABI
            .function("swap")
            .unwrap()
            .encode_input(&input_tokens)
            .expect("Could not encode swap calldata")
    }
}

#[cfg(test)]
mod tests {
    use std::{str::FromStr, sync::Arc};

    use ethers::{
        providers::{Http, Provider},
        types::{H160, U256},
    };

    use super::UniswapV2Pool;

    #[test]
    fn test_swap_calldata() {
        let uniswap_v2_pool = UniswapV2Pool::default();

        let _calldata = uniswap_v2_pool.swap_calldata(
            U256::from(123456789),
            U256::zero(),
            H160::from_str("0x41c36f504BE664982e7519480409Caf36EE4f008").unwrap(),
            vec![],
        );
    }

    #[tokio::test]
    async fn test_get_new_from_address() {
        let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")
            .expect("Could not get ETHEREUM_RPC_ENDPOINT");
        let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint).unwrap());

        let pool = UniswapV2Pool::new_from_address(
            H160::from_str("0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc").unwrap(),
            middleware.clone(),
        )
        .await
        .unwrap();

        assert_eq!(
            pool.address,
            H160::from_str("0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc").unwrap()
        );
        assert_eq!(
            pool.token_a,
            H160::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap()
        );
        assert_eq!(pool.token_a_decimals, 6);
        assert_eq!(
            pool.token_b,
            H160::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap()
        );
        assert_eq!(pool.token_b_decimals, 18);
        assert_eq!(pool.fee, 300);
    }

    #[tokio::test]
    async fn test_get_pool_data() {
        let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")
            .expect("Could not get ETHEREUM_RPC_ENDPOINT");
        let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint).unwrap());

        let mut pool = UniswapV2Pool {
            address: H160::from_str("0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc").unwrap(),
            ..Default::default()
        };

        pool.get_pool_data(middleware).await.unwrap();

        assert_eq!(
            pool.address,
            H160::from_str("0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc").unwrap()
        );
        assert_eq!(
            pool.token_a,
            H160::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap()
        );
        assert_eq!(pool.token_a_decimals, 6);
        assert_eq!(
            pool.token_b,
            H160::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap()
        );
        assert_eq!(pool.token_b_decimals, 18);
        assert_eq!(pool.fee, 300);
    }

    #[tokio::test]
    async fn test_calculate_price_64_x_64() {
        let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")
            .expect("Could not get ETHEREUM_RPC_ENDPOINT");
        let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint).unwrap());

        let mut pool = UniswapV2Pool {
            address: H160::from_str("0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc").unwrap(),
            ..Default::default()
        };

        pool.get_pool_data(middleware.clone()).await.unwrap();

        pool.reserve_0 = 47092140895915;
        pool.reserve_1 = 28396598565590008529300;

        let price_a_64_x = pool.calculate_price_64_x_64(pool.token_a).unwrap();

        let price_b_64_x = pool.calculate_price_64_x_64(pool.token_b).unwrap();

        assert_eq!(30591574867092394336528, price_b_64_x);
        assert_eq!(11123401407064628, price_a_64_x);
    }
}
