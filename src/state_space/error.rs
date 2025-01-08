use alloy::transports::TransportErrorKind;
use thiserror::Error;

use crate::amms::error::AMMError;

#[derive(Error, Debug)]
pub enum StateSpaceError {
    #[error(transparent)]
    AMMError(#[from] AMMError),
    #[error(transparent)]
    TransportError(#[from] alloy::transports::RpcError<TransportErrorKind>),
    #[error(transparent)]
    JoinError(#[from] tokio::task::JoinError),
    #[error("Block Number Does not Exist")]
    MissingBlockNumber,
    #[error(transparent)]
    CheckpointError(#[from] serde_json::Error),
}
