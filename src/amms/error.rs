use thiserror::Error;

#[derive(Error, Debug)]
pub enum AMMError {
    #[error(transparent)]
    UniswapV3MathError(#[from] uniswap_v3_math::error::UniswapV3MathError),
    #[error("Liquidity Underflow")]
    LiquidityUnderflow,
    #[error("Parse Float Error")]
    ParseFloatError,
    #[error("Division by zero")]
    DivisionByZero,
    #[error("Rounding Error")]
    RoundingError,
}
