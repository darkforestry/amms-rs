use alloy::primitives::{Address, U256};
use alloy::transports::TransportError;

use std::time::SystemTimeError;
use thiserror::Error;
use tokio::task::JoinError;
use uniswap_v3_math::error::UniswapV3MathError;

#[derive(Error, Debug)]
pub enum AMMError {
    #[error(transparent)]
    TransportError(#[from] TransportError),
    #[error(transparent)]
    ContractError(#[from] alloy::contract::Error),
    #[error(transparent)]
    ABICodecError(#[from] alloy::dyn_abi::Error),
    #[error(transparent)]
    EthABIError(#[from] alloy::sol_types::Error),
    #[error(transparent)]
    JoinError(#[from] JoinError),
    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::error::Error),
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error("Error when converting from hex to U256")]
    FromHexError,
    #[error(transparent)]
    UniswapV3MathError(#[from] UniswapV3MathError),
    #[error("Pair for token_a/token_b does not exist in provided dexes")]
    PairDoesNotExistInDexes(Address, Address),
    #[error("Could not initialize new pool from event log")]
    UnrecognizedPoolCreatedEventLog,
    #[error("Error when syncing pool")]
    SyncError(Address),
    #[error("Error when getting pool data")]
    PoolDataError,
    #[error(transparent)]
    ArithmeticError(#[from] ArithmeticError),
    #[error("No initialized ticks during v3 swap simulation")]
    NoInitializedTicks,
    #[error("No liquidity net found during v3 swap simulation")]
    NoLiquidityNet,
    #[error("Incongruent AMMS supplied to batch request")]
    IncongruentAMMs,
    #[error("Invalid ERC4626 fee")]
    InvalidERC4626Fee,
    #[error(transparent)]
    EventLogError(#[from] EventLogError),
    #[error("Block number not found")]
    BlockNumberNotFound,
    #[error(transparent)]
    SwapSimulationError(#[from] SwapSimulationError),
    #[error("Invalid data from batch request")]
    BatchRequestError(Address),
    #[error(transparent)]
    CheckpointError(#[from] CheckpointError),
    #[error(transparent)]
    EyreError(#[from] eyre::Error),
}

#[derive(Error, Debug)]
pub enum ArithmeticError {
    #[error("Shadow overflow")]
    ShadowOverflow(U256),
    #[error("Rounding Error")]
    RoundingError,
    #[error("Y is zero")]
    YIsZero,
    #[error("Sqrt price overflow")]
    SqrtPriceOverflow,
    #[error("U128 conversion error")]
    U128ConversionError,
    #[error(transparent)]
    UniswapV3MathError(#[from] UniswapV3MathError),
}

#[derive(Error, Debug)]
pub enum EventLogError {
    #[error("Invalid event signature")]
    InvalidEventSignature,
    #[error("Log Block number not found")]
    LogBlockNumberNotFound,
    #[error(transparent)]
    EthABIError(#[from] alloy::sol_types::Error),
    #[error(transparent)]
    ABIError(#[from] alloy::dyn_abi::Error),
}

#[derive(Error, Debug)]
pub enum SwapSimulationError {
    #[error("Could not get next tick")]
    InvalidTick,
    #[error(transparent)]
    UniswapV3MathError(#[from] UniswapV3MathError),
    #[error("Liquidity underflow")]
    LiquidityUnderflow,
}

#[derive(Error, Debug)]
pub enum CheckpointError {
    #[error(transparent)]
    SystemTimeError(#[from] SystemTimeError),
    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::error::Error),
    #[error(transparent)]
    IOError(#[from] std::io::Error),
}
