use alloy::{primitives::{Address, FixedBytes}, transports::TransportErrorKind};
use thiserror::Error;
use uniswap_v3_math::error::UniswapV3MathError;

#[derive(Error, Debug)]
pub enum AMMError {
    #[error(transparent)]
    TransportError(#[from] alloy_json_rpc::RpcError<TransportErrorKind>),
    #[error(transparent)]
    ContractError(#[from] alloy::contract::Error),
    #[error(transparent)]
    ABIError(#[from] alloy::dyn_abi::Error),
    #[error(transparent)]
    SolTypesError(#[from] alloy::sol_types::Error),
    #[error(transparent)]
    UniswapV2Error(#[from] UniswapV2Error),
    #[error(transparent)]
    UniswapV3Error(#[from] UniswapV3Error),
    #[error("Invalid AMM Address")]
    InvalidAMMAddress(Address),
    #[error("Unknown Event Signature {0}")]
    UnknownEventSignature(FixedBytes<32>),
}

#[derive(Error, Debug)]
pub enum UniswapV2Error {
    #[error("Parse Float Error")]
    ParseFloatError,
    #[error("Division by zero")]
    DivisionByZero,
    #[error("Rounding Error")]
    RoundingError,
}

#[derive(Error, Debug)]
pub enum UniswapV3Error {
    #[error(transparent)]
    UniswapV3MathError(#[from] UniswapV3MathError),
    #[error("Liquidity Underflow")]
    LiquidityUnderflow,
    #[error("Tick Does Not Exist {0}")]
    TickNotFound(i32),
}
