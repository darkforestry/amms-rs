use thiserror::Error;

#[derive(Error, Debug)]
pub enum BMathError {
    #[error("Division by zero")]
    DivZero,
    #[error("Error during division")]
    DivInternal,
    #[error("Addition overflow")]
    AddOverflow,
    #[error("Subtraction underflow")]
    SubUnderflow,
    #[error("Multiplication overflow")]
    MulOverflow,
}
