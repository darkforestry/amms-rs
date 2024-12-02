use alloy::transports::TransportErrorKind;
use thiserror::Error;

use crate::amms::error::AMMError;

#[derive(Error, Debug)]
pub enum StateSpaceError {
    #[error(transparent)]
    AMMError(#[from] AMMError),
    #[error(transparent)]
    TransportError(#[from] alloy_json_rpc::RpcError<TransportErrorKind>),
    #[error(transparent)]
    JoinError(#[from] tokio::task::JoinError),
    #[error(transparent)]
    ErrReport(#[from] eyre::Report),
    #[error("Logs Are Empty")]
    MissingLogs,
    #[error("Block Number Does not Exist")]
    MissingBlockNumber,
}
