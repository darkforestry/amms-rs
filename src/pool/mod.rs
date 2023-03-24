use std::{cmp::Ordering, sync::Arc};

use ethers::{
    providers::Middleware,
    types::{Log, H160, U256},
};
use serde::{Deserialize, Serialize};

use crate::{
    dex::{self, DexVariant},
    errors::{ArithmeticError, CFMMError},
};

pub mod fixed_point_math;
pub mod uniswap_v2;
pub mod uniswap_v3;
pub use uniswap_v2::UniswapV2Pool;
pub use uniswap_v3::UniswapV3Pool;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Pool {
    UniswapV2(UniswapV2Pool),
    UniswapV3(UniswapV3Pool),
}

impl Pool {
    //Creates a new pool with all pool data populated from the pair address.
    pub async fn new_from_address<M: Middleware>(
        pair_address: H160,
        dex_variant: DexVariant,
        middleware: Arc<M>,
    ) -> Result<Self, CFMMError<M>> {
        match dex_variant {
            DexVariant::UniswapV2 => Ok(Pool::UniswapV2(
                UniswapV2Pool::new_from_address(pair_address, middleware).await?,
            )),

            DexVariant::UniswapV3 => Ok(Pool::UniswapV3(
                UniswapV3Pool::new_from_address(pair_address, middleware).await?,
            )),
        }
    }

    pub fn fee(&self) -> u32 {
        match self {
            Pool::UniswapV2(pool) => pool.fee(),
            Pool::UniswapV3(pool) => pool.fee(),
        }
    }

    //Creates a new pool with all pool data populated from the pair address.
    pub async fn new_from_event_log<M: Middleware>(
        log: Log,
        middleware: Arc<M>,
    ) -> Result<Self, CFMMError<M>> {
        let event_signature = log.topics[0];

        if event_signature == dex::uniswap_v2::PAIR_CREATED_EVENT_SIGNATURE {
            Ok(Pool::UniswapV2(
                UniswapV2Pool::new_from_event_log(log, middleware).await?,
            ))
        } else if event_signature == dex::uniswap_v3::POOL_CREATED_EVENT_SIGNATURE {
            Ok(Pool::UniswapV3(
                UniswapV3Pool::new_from_event_log(log, middleware).await?,
            ))
        } else {
            Err(CFMMError::UnrecognizedPoolCreatedEventLog)
        }
    }

    //Creates a new pool with all pool data populated from the pair address.
    pub fn new_empty_pool_from_event_log<M: Middleware>(log: Log) -> Result<Self, CFMMError<M>> {
        let event_signature = log.topics[0];

        if event_signature == dex::uniswap_v2::PAIR_CREATED_EVENT_SIGNATURE {
            Ok(Pool::UniswapV2(
                UniswapV2Pool::new_empty_pool_from_event_log(log)?,
            ))
        } else if event_signature == dex::uniswap_v3::POOL_CREATED_EVENT_SIGNATURE {
            Ok(Pool::UniswapV3(
                UniswapV3Pool::new_empty_pool_from_event_log(log)?,
            ))
        } else {
            Err(CFMMError::UnrecognizedPoolCreatedEventLog)
        }
    }

    pub async fn sync_pool<M: Middleware>(
        &mut self,
        middleware: Arc<M>,
    ) -> Result<(), CFMMError<M>> {
        match self {
            Pool::UniswapV2(pool) => pool.sync_pool(middleware).await,
            Pool::UniswapV3(pool) => pool.sync_pool(middleware).await,
        }
    }

    //Get price of base token per pair token
    pub fn calculate_price(&self, base_token: H160) -> Result<f64, ArithmeticError> {
        match self {
            Pool::UniswapV2(pool) => pool.calculate_price(base_token),
            Pool::UniswapV3(pool) => Ok(pool.calculate_price(base_token)),
        }
    }

    pub async fn get_pool_data<M: Middleware>(
        &mut self,
        middleware: Arc<M>,
    ) -> Result<(), CFMMError<M>> {
        match self {
            Pool::UniswapV2(pool) => pool.get_pool_data(middleware).await?,
            Pool::UniswapV3(pool) => pool.get_pool_data(middleware).await?,
        }
        Ok(())
    }

    pub fn address(&self) -> H160 {
        match self {
            Pool::UniswapV2(pool) => pool.address(),
            Pool::UniswapV3(pool) => pool.address(),
        }
    }

    pub async fn simulate_swap<M: Middleware>(
        &self,
        token_in: H160,
        amount_in: U256,
        middleware: Arc<M>,
    ) -> Result<U256, CFMMError<M>> {
        match self {
            Pool::UniswapV2(pool) => Ok(pool.simulate_swap(token_in, amount_in)),
            Pool::UniswapV3(pool) => pool.simulate_swap(token_in, amount_in, middleware).await,
        }
    }

    pub async fn simulate_swap_mut<M: Middleware>(
        &mut self,
        token_in: H160,
        amount_in: U256,
        middleware: Arc<M>,
    ) -> Result<U256, CFMMError<M>> {
        match self {
            Pool::UniswapV2(pool) => Ok(pool.simulate_swap_mut(token_in, amount_in)),
            Pool::UniswapV3(pool) => {
                pool.simulate_swap_mut(token_in, amount_in, middleware)
                    .await
            }
        }
    }
}

pub fn convert_to_decimals(amount: U256, decimals: u8, target_decimals: u8) -> U256 {
    match target_decimals.cmp(&decimals) {
        Ordering::Less => amount / U256::from(10u128.pow((decimals - target_decimals) as u32)),
        Ordering::Greater => amount * U256::from(10u128.pow((target_decimals - decimals) as u32)),
        _ => amount,
    }
}

pub fn convert_to_common_decimals(
    amount_a: U256,
    a_decimals: u8,
    amount_b: U256,
    b_decimals: u8,
) -> (U256, U256, u8) {
    match a_decimals.cmp(&b_decimals) {
        Ordering::Less => {
            let amount_a = convert_to_decimals(amount_a, a_decimals, b_decimals);
            (amount_a, amount_b, b_decimals)
        }
        Ordering::Greater => {
            let amount_b = convert_to_decimals(amount_b, b_decimals, a_decimals);
            (amount_a, amount_b, a_decimals)
        }
        Ordering::Equal => (amount_a, amount_b, a_decimals),
    }
}

pub async fn simulate_route<M: Middleware>(
    mut token_in: H160,
    mut amount_in: U256,
    route: &[Pool],
    middleware: Arc<M>,
) -> Result<U256, CFMMError<M>> {
    let mut amount_out = U256::zero();

    for pool in route {
        amount_out = pool
            .simulate_swap(token_in, amount_in, middleware.clone())
            .await?;

        token_in = match pool {
            Pool::UniswapV2(pool) => {
                if token_in == pool.token_a {
                    pool.token_b
                } else {
                    pool.token_a
                }
            }

            Pool::UniswapV3(pool) => {
                if token_in == pool.token_a {
                    pool.token_b
                } else {
                    pool.token_a
                }
            }
        };

        amount_in = amount_out
    }

    Ok(amount_out)
}

pub async fn simulate_route_mut<M: Middleware>(
    mut token_in: H160,
    mut amount_in: U256,
    route: &mut [Pool],
    middleware: Arc<M>,
) -> Result<U256, CFMMError<M>> {
    let mut amount_out = U256::zero();

    for pool in route {
        amount_out = pool
            .simulate_swap_mut(token_in, amount_in, middleware.clone())
            .await?;

        token_in = match pool {
            Pool::UniswapV2(pool) => {
                if token_in == pool.token_a {
                    pool.token_b
                } else {
                    pool.token_a
                }
            }

            Pool::UniswapV3(pool) => {
                if token_in == pool.token_a {
                    pool.token_b
                } else {
                    pool.token_a
                }
            }
        };

        amount_in = amount_out
    }

    Ok(amount_out)
}
