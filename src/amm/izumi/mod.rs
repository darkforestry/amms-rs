use std::{cmp::Ordering, str::FromStr, sync::Arc};

use async_trait::async_trait;
use ethers::{
    abi::{ethabi::Bytes, RawLog, Token},
    prelude::{abigen, EthEvent},
    providers::Middleware,
    types::{Log, H160, H256, I256, U256},
};
use serde::{Deserialize, Serialize};

use crate::errors::{ArithmeticError, DAMMError, EventLogError, SwapSimulationError};

use self::factory::{NewPoolFilter, IZI_POOL_CREATED_EVENT_SIGNATURE};

use super::AutomatedMarketMaker;
pub mod batch_request;
pub mod factory;

abigen!(

    IiZiSwapPool,
    r#"[
        function token0() external view returns (address)
        function token1() external view returns (address)
        function liquidity() external view returns (uint128)
        function fee() external view returns (uint24)
        function swapY2X(address recipient,uint128 amount,int24 highPt,bytes calldata data) returns (uint256 amountX, uint256 amountY)          
        function swapX2Y(address recipient,uint128 amount,int24 lowPt,bytes calldata data) returns (uint256 amountX, uint256 amountY)
        event Swap(address indexed tokenX,address indexed tokenY,uint24 indexed fee,bool sellXEarnY,uint256 amountX,uint256 amountY)
    ]"#;

    IErc20,
    r#"[
        function balanceOf(address account) external view returns (uint256)
        function decimals() external view returns (uint8)
    ]"#;

    IQuoter,
    r#"[
        function swapY2X(address tokenX,address tokenY,uint24 fee,uint128 amount,int24 highPt) public returns (uint256 amountX, int24 finalPoint)
        function swapX2Y(address tokenX,address tokenY,uint24 fee,uint128 amount,int24 lowPt) public returns (uint256 amountY, int24 finalPoint)
    ]"#
);

pub const SWAP_EVENT_SIGNATURE: H256 = H256([
    231, 119, 154, 54, 162, 138, 224, 228, 155, 203, 217, 252, 245, 114, 134, 251, 96, 118, 153,
    192, 195, 57, 194, 2, 233, 36, 149, 100, 5, 5, 97, 62,
]);

pub const MIN_PT: i32 = -800000;
pub const MAX_PT: i32 = 800000;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IziSwapPool {
    pub address: H160,
    pub token_a: H160,
    pub token_a_decimals: u8,
    pub token_b: H160,
    pub token_b_decimals: u8,
    pub liquidity: u128,
    pub liquidity_x: u128,
    pub liquidity_y: u128,
    pub sqrt_price: U256,
    pub fee: u32,
    pub current_point: i32,
    pub point_delta: i32,
}
#[async_trait]
impl AutomatedMarketMaker for IziSwapPool {
    fn address(&self) -> H160 {
        self.address
    }
    async fn sync<M: Middleware>(&mut self, middleware: Arc<M>) -> Result<(), DAMMError<M>> {
        batch_request::sync_izi_pool_batch_request(self, middleware.clone()).await?;
        Ok(())
    }
    fn sync_on_event_signatures(&self) -> Vec<H256> {
        vec![SWAP_EVENT_SIGNATURE]
    }
    fn tokens(&self) -> Vec<H160> {
        vec![self.token_a, self.token_b]
    }
    fn calculate_price(&self, base_token: H160) -> Result<f64, ArithmeticError> {
        let shift = self.token_a_decimals as i8 - self.token_b_decimals as i8;
        let price = match shift.cmp(&0) {
            Ordering::Less => 1.0001_f64.powi(self.current_point) / 10_f64.powi(-shift as i32),
            Ordering::Greater => 1.0001_f64.powi(self.current_point) * 10_f64.powi(shift as i32),
            Ordering::Equal => 1.0001_f64.powi(self.current_point),
        };
        if base_token == self.token_a {
            Ok(price)
        } else {
            Ok(1.0 / price)
        }
    }
    fn sync_from_log(&mut self, _log: ethers::types::Log) -> Result<(), EventLogError> {
        todo!("Not yet implemented");
    }
    async fn populate_data<M: Middleware>(
        &mut self,
        block_number: Option<u64>,
        middleware: Arc<M>,
    ) -> Result<(), DAMMError<M>> {
        batch_request::get_izi_pool_data_batch_request(self, block_number, middleware.clone())
            .await?;
        Ok(())
    }
    fn simulate_swap(
        &self,
        _token_in: H160,
        _amount_in: U256,
    ) -> Result<U256, SwapSimulationError> {
        todo!("Not yet implemented");
    }
    fn simulate_swap_mut(
        &mut self,
        _token_in: H160,
        _amount_in: U256,
    ) -> Result<U256, SwapSimulationError> {
        todo!("Not yet implemented");
    }

    fn get_token_out(&self, token_in: H160) -> H160 {
        if self.token_a == token_in {
            self.token_b
        } else {
            self.token_a
        }
    }
}

impl IziSwapPool {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        address: H160,
        token_a: H160,
        token_a_decimals: u8,
        token_b: H160,
        token_b_decimals: u8,
        fee: u32,
        liquidity: u128,
        sqrt_price: U256,
        liquidity_x: u128,
        liquidity_y: u128,
        current_point: i32,
        point_delta: i32,
    ) -> IziSwapPool {
        IziSwapPool {
            address,
            token_a,
            token_a_decimals,
            token_b,
            token_b_decimals,
            fee,
            liquidity,
            sqrt_price,
            liquidity_x,
            liquidity_y,
            current_point,
            point_delta,
        }
    }

    //TODO: document that this function will not populate the tick_bitmap and ticks, if you want to populate those, you must call populate_tick_data on an initialized pool.1.0001_f64

    //Creates a new instance of the pool from the pair address
    pub async fn new_from_address<M: 'static + Middleware>(
        pair_address: H160,
        _creation_block: u64,
        middleware: Arc<M>,
    ) -> Result<Self, DAMMError<M>> {
        let mut pool = IziSwapPool {
            address: pair_address,
            token_a: H160::zero(),
            token_a_decimals: 0,
            token_b: H160::zero(),
            token_b_decimals: 0,
            liquidity: 0,
            sqrt_price: U256::zero(),
            liquidity_x: 0,
            liquidity_y: 0,
            current_point: 0,
            point_delta: 0,
            fee: 0,
        };

        //TODO: break this into two threads so it can happen concurrently?
        pool.populate_data(None, middleware).await?;

        if !pool.data_is_populated() {
            return Err(DAMMError::PoolDataError);
        }

        Ok(pool)
    }

    pub async fn new_from_log<M: 'static + Middleware>(
        log: Log,
        middleware: Arc<M>,
    ) -> Result<Self, DAMMError<M>> {
        let event_signature = log.topics[0];

        if event_signature == IZI_POOL_CREATED_EVENT_SIGNATURE {
            if let Some(block_number) = log.block_number {
                let pool_created_event = NewPoolFilter::decode_log(&RawLog::from(log))?;

                IziSwapPool::new_from_address(
                    pool_created_event.pool,
                    block_number.as_u64(),
                    middleware,
                )
                .await
            } else {
                Err(EventLogError::LogBlockNumberNotFound)?
            }
        } else {
            Err(EventLogError::InvalidEventSignature)?
        }
    }

    pub fn new_empty_pool_from_log(log: Log) -> Result<Self, EventLogError> {
        let event_signature = log.topics[0];

        if event_signature == IZI_POOL_CREATED_EVENT_SIGNATURE {
            let pool_created_event = NewPoolFilter::decode_log(&RawLog::from(log))?;

            Ok(IziSwapPool {
                address: pool_created_event.pool,
                token_a: pool_created_event.token_x,
                token_b: pool_created_event.token_y,
                token_a_decimals: 0,
                token_b_decimals: 0,
                fee: pool_created_event.fee,
                liquidity: 0,
                sqrt_price: U256::zero(),
                liquidity_x: 0,
                liquidity_y: 0,
                current_point: 0,
                point_delta: 0,
            })
        } else {
            Err(EventLogError::InvalidEventSignature)
        }
    }

    pub fn fee(&self) -> u32 {
        self.fee
    }

    pub fn data_is_populated(&self) -> bool {
        !(self.token_a.is_zero() || self.token_b.is_zero())
    }

    pub async fn sync_from_swap_log<M: Middleware>(
        &mut self,
        _log: Log,
        middleware: Arc<M>,
    ) -> Result<(), DAMMError<M>> {
        batch_request::sync_izi_pool_batch_request(self, middleware).await?;
        Ok(())
    }

    pub async fn get_token_decimals<M: Middleware>(
        &mut self,
        middleware: Arc<M>,
    ) -> Result<(u8, u8), DAMMError<M>> {
        let token_a_decimals = IErc20::new(self.token_a, middleware.clone())
            .decimals()
            .call()
            .await?;

        let token_b_decimals = IErc20::new(self.token_b, middleware)
            .decimals()
            .call()
            .await?;

        Ok((token_a_decimals, token_b_decimals))
    }

    pub async fn simulate_swap_async<M: Middleware>(
        &self,
        token_in: H160,
        amount_in: u128,
        quoter: &str,
        middleware: Arc<M>,
    ) -> Result<U256, DAMMError<M>> {
        let quoter = IQuoter::new(H160::from_str(quoter).unwrap(), middleware.clone());

        if token_in == self.token_a {
            let (amount_out, _) = quoter
                .swap_x2y(token_in, self.token_b, self.fee, amount_in, MIN_PT)
                .call()
                .await?;
            Ok(amount_out)
        } else {
            let (amount_out, _) = quoter
                .swap_y2x(token_in, self.token_a, self.fee, amount_in, MAX_PT)
                .call()
                .await?;
            Ok(amount_out)
        }
    }

    pub fn swap_calldata(
        &self,
        recipient: H160,
        zero_for_one: bool,
        amount: U256,
        limit_pt: I256,
        calldata: Vec<u8>,
    ) -> Bytes {
        let input_tokens = vec![
            Token::Address(recipient),
            Token::Uint(amount),
            Token::Int(limit_pt.into_raw()),
            Token::Bytes(calldata),
        ];
        if zero_for_one {
            IIZISWAPPOOL_ABI
                .function("swapX2Y")
                .unwrap()
                .encode_input(&input_tokens)
                .expect("Could not encode swap calldata")
        } else {
            IIZISWAPPOOL_ABI
                .function("swapY2X")
                .unwrap()
                .encode_input(&input_tokens)
                .expect("Could not encode swap calldata")
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{
        amm::{izumi::IziSwapPool, AutomatedMarketMaker},
        errors::DAMMError,
    };

    #[allow(unused)]
    use ethers::providers::Middleware;

    use ethers::types::H256;
    #[allow(unused)]
    use ethers::{
        prelude::abigen,
        providers::{Http, Provider},
        types::{H160, U256},
    };
    #[allow(unused)]
    use std::error::Error;
    #[allow(unused)]
    use std::{str::FromStr, sync::Arc};
    abigen!(
        IQuoter,
    r#"[
        function quoteExactInputSingle(address tokenIn, address tokenOut,uint24 fee, uint256 amountIn, uint160 sqrtPriceLimitX96) external returns (uint256 amountOut)
    ]"#;);

    #[tokio::test]
    async fn test_get_new_from_address() {
        let rpc_endpoint = std::env::var("ARBITRUM_MAINNET_ENDPOINT")
            .expect("Could not get ETHEREUM_RPC_ENDPOINT");
        let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint).unwrap());

        let pool = IziSwapPool::new_from_address(
            H160::from_str("0x6336e3F52d196b4f63eE512455237c934B3355eB").unwrap(),
            29420590,
            middleware.clone(),
        )
        .await
        .unwrap();

        assert_eq!(
            pool.address,
            H160::from_str("0x6336e3F52d196b4f63eE512455237c934B3355eB").unwrap()
        );
        assert_eq!(
            pool.token_a,
            H160::from_str("0x82af49447d8a07e3bd95bd0d56f35241523fbab1").unwrap()
        );
        assert_eq!(pool.token_a_decimals, 18);
        assert_eq!(
            pool.token_b,
            H160::from_str("0xff970a61a04b1ca14834a43f5de4533ebddb5cc8").unwrap()
        );
        assert_eq!(pool.token_b_decimals, 6);
        assert_eq!(pool.fee, 2000);
        assert_eq!(pool.point_delta, 40);
    }

    #[tokio::test]
    async fn test_get_pool_data() {
        let rpc_endpoint = std::env::var("ARBITRUM_MAINNET_ENDPOINT")
            .expect("Could not get ETHEREUM_RPC_ENDPOINT");
        let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint).unwrap());

        let mut pool = IziSwapPool::new_from_address(
            H160::from_str("0x6336e3F52d196b4f63eE512455237c934B3355eB").unwrap(),
            29420590,
            middleware.clone(),
        )
        .await
        .expect("Could not initialize pool");
        let current_block = middleware.get_block_number().await.unwrap().as_u64();
        pool.populate_data(Some(current_block), middleware);
        assert_eq!(
            pool.address,
            H160::from_str("0x6336e3F52d196b4f63eE512455237c934B3355eB").unwrap()
        );
        assert_eq!(
            pool.token_a,
            H160::from_str("0x82af49447d8a07e3bd95bd0d56f35241523fbab1").unwrap()
        );
        assert_eq!(pool.token_a_decimals, 18);
        assert_eq!(
            pool.token_b,
            H160::from_str("0xff970a61a04b1ca14834a43f5de4533ebddb5cc8").unwrap()
        );
        assert_eq!(pool.token_b_decimals, 6);
        assert_eq!(pool.fee, 2000);
        assert_eq!(pool.point_delta, 40);
        assert!(pool.sqrt_price != U256::zero());
    }

    #[tokio::test]
    async fn test_sync_pool() {
        let rpc_endpoint = std::env::var("ARBITRUM_MAINNET_ENDPOINT")
            .expect("Could not get ETHEREUM_RPC_ENDPOINT");
        let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint).unwrap());

        let mut pool = IziSwapPool::new_from_address(
            H160::from_str("0x6336e3F52d196b4f63eE512455237c934B3355eB").unwrap(),
            29420590,
            middleware.clone(),
        )
        .await
        .expect("Could not initialize pool");

        
        dbg!(pool.address);

        pool.sync(middleware).await;

        //TODO: need to assert values
    }

    // #[tokio::test]
    // async fn test_calculate_price() {
    //     let rpc_endpoint = std::env::var("ARBITRUM_MAINNET_ENDPOINT")
    //         .expect("Could not get ETHEREUM_RPC_ENDPOINT");
    //     let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint).unwrap());

    //     let mut pool = IziSwapPool {
    //         address: H160::from_str("0x6336e3F52d196b4f63eE512455237c934B3355eB").unwrap(),
    //         ..Default::default()
    //     };

    //     pool.populate_data(None, middleware.clone()).await.unwrap();

    //     let sqrt_price = block_pool.slot_0().block(16515398).call().await.unwrap().0;
    //     pool.sqrt_price = sqrt_price;
    // }
}
