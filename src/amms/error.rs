use alloy::{primitives::FixedBytes, transports::TransportErrorKind};
use thiserror::Error;

use super::{
    balancer::BalancerError, erc_4626::ERC4626VaultError, uniswap_v2::UniswapV2Error,
    uniswap_v3::UniswapV3Error,
};

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
    #[error(transparent)]
    BalancerError(#[from] BalancerError),
    #[error(transparent)]
    ERC4626VaultError(#[from] ERC4626VaultError),
    #[error(transparent)]
    ParseFloatError(#[from] rug::float::ParseFloatError),
    #[error("Unrecognized Event Signature {0}")]
    UnrecognizedEventSignature(FixedBytes<32>),
    #[error(transparent)]
    JoinError(#[from] tokio::task::JoinError),
}
