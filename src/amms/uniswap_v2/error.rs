use thiserror::Error;

#[derive(Error, Debug)]
pub enum UniswapV2Error {
    #[error("Division by zero")]
    DivisionByZero,
    #[error("Rounding Error")]
    RoundingError,
}