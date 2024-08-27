use crate::errors::{AMMError, ArithmeticError, EventLogError};

use alloy::{primitives::Address, rpc::types::eth::Block, transports::TransportError};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum StateSpaceError {
    #[error(transparent)]
    TransportError(#[from] TransportError),
    #[error(transparent)]
    ContractError(#[from] alloy::contract::Error),
    #[error(transparent)]
    ABICodecError(#[from] alloy::dyn_abi::Error),
    #[error(transparent)]
    EthABIError(#[from] alloy::sol_types::Error),
    #[error(transparent)]
    AMMError(#[from] AMMError),
    #[error(transparent)]
    ArithmeticError(#[from] ArithmeticError),
    #[error(transparent)]
    WalletError(#[from] alloy::signers::local::LocalSignerError),
    #[error("Insufficient wallet funds for execution")]
    InsufficientWalletFunds(),
    #[error(transparent)]
    EventLogError(#[from] EventLogError),
    #[error("Block number not found")]
    BlockNumberNotFound,
    #[error(transparent)]
    StateChangeSendError(#[from] tokio::sync::mpsc::error::SendError<Vec<Address>>),
    #[error(transparent)]
    BlockSendError(#[from] tokio::sync::mpsc::error::SendError<Block>),
    #[error("Already listening for state changes")]
    AlreadyListeningForStateChanges,
    #[error(transparent)]
    JoinError(#[from] tokio::task::JoinError),
}
