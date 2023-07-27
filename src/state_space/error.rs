use crate::errors::{AMMError, ArithmeticError, EventLogError};

use ethers::prelude::{AbiError, ContractError};

use ethers::providers::{Middleware, ProviderError};

use ethers::signers::WalletError;
use ethers::types::{Block, H160, H256};
use thiserror::Error;

use super::state::MiddlewarePubsub;

#[derive(Error, Debug)]
pub enum StateSpaceError<M, P>
where
    M: Middleware,
    P: MiddlewarePubsub,
{
    #[error("Middleware error")]
    MiddlewareError(<M as Middleware>::Error),
    #[error("Pubsub client error")]
    PubsubClientError(<P as Middleware>::Error),
    #[error("Provider error")]
    ProviderError(#[from] ProviderError),
    #[error("Contract error")]
    ContractError(#[from] ContractError<M>),
    #[error("ABI Codec error")]
    ABICodecError(#[from] AbiError),
    #[error("Eth ABI error")]
    EthABIError(#[from] ethers::abi::Error),
    #[error("AMM error")]
    AMMError(#[from] AMMError<M>),
    #[error("Arithmetic error")]
    ArithmeticError(#[from] ArithmeticError),
    #[error("Wallet error")]
    WalletError(#[from] WalletError),
    #[error("Insufficient wallet funds for execution")]
    InsufficientWalletFunds(),
    #[error("Event log error")]
    EventLogError(#[from] EventLogError),
    #[error("State change error")]
    StateChangeError(#[from] StateChangeError),
    #[error("Block number not found")]
    BlockNumberNotFound,
    #[error("Could not send state changes through channel")]
    StateChangeSendError(#[from] tokio::sync::mpsc::error::SendError<Vec<H160>>),
    #[error("Could not send block through channel")]
    BlockSendError(#[from] tokio::sync::mpsc::error::SendError<Block<H256>>),
    #[error("Already listening for state changes")]
    AlreadyListeningForStateChanges,
}

#[derive(Error, Debug)]
pub enum StateChangeError {
    #[error("No state changes in cache")]
    NoStateChangesInCache,
    #[error("Error when removing a state change from the front of the deque")]
    PopFrontError,
    #[error("State change cache capacity error")]
    CapacityError,
    #[error("Event log error")]
    EventLogError(#[from] EventLogError),
}
