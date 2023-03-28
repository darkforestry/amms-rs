use std::sync::Arc;

use ethers::{
    providers::Middleware,
    types::{H160, H256},
};
use serde::{Deserialize, Serialize};

use crate::errors::{ArithmeticError, DAMMError};

use super::AutomatedMarketMaker;

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct X {}
