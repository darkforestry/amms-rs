use std::fmt;

use crate::errors::{AMMError, ArithmeticError, EventLogError};

use alloy::{network::Network, primitives::Address, transports::TransportError};

use arraydeque::CapacityError;
use thiserror::Error;

use super::StateChange;

// Define newtype wrappers to distinguish between the SendErrors
#[derive(Debug)]
pub struct StateChangeSendErrorWrapper(pub tokio::sync::mpsc::error::SendError<Vec<Address>>);

#[derive(Debug)]
pub struct BlockSendErrorWrapper<N: Network>(
    pub tokio::sync::mpsc::error::SendError<<N as alloy::providers::Network>::BlockResponse>,
);

// Implement Display for StateChangeSendErrorWrapper
impl fmt::Display for StateChangeSendErrorWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StateChange send error: {}", self.0)
    }
}

// Implement Error for StateChangeSendErrorWrapper
impl std::error::Error for StateChangeSendErrorWrapper {}

// Implement Display for BlockSendErrorWrapper
impl<N: Network> fmt::Display for BlockSendErrorWrapper<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Block send error: {}", self.0)
    }
}

// Implement Error for BlockSendErrorWrapper
impl<N: Network> std::error::Error for BlockSendErrorWrapper<N> {}

#[derive(Error, Debug)]
pub enum StateSpaceError<N: Network> {
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
    StateChangeSendError(#[from] StateChangeSendErrorWrapper),
    #[error(transparent)]
    CapacityError(#[from] CapacityError<StateChange>),
    #[error(transparent)]
    BlockSendError(#[from] BlockSendErrorWrapper<N>),
    #[error("Already listening for state changes")]
    AlreadyListeningForStateChanges,
    #[error(transparent)]
    JoinError(#[from] tokio::task::JoinError),
}
