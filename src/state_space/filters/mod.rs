pub mod blacklist;
pub mod value;
pub mod whitelist;

use async_trait::async_trait;
use blacklist::BlacklistFilter;
use eyre::Result;
use whitelist::{PoolWhitelistFilter, TokenWhitelistFilter};

use crate::amms::amm::AMM;
#[async_trait]
pub trait AMMFilter {
    async fn filter(&self, amms: Vec<AMM>) -> Result<Vec<AMM>>;
    fn stage(&self) -> FilterStage;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterStage {
    Discovery,
    Sync,
}

macro_rules! filter {
    ($($filter_type:ident),+ $(,)?) => {
        #[derive(Debug, Clone)]
        pub enum PoolFilter {
            $($filter_type($filter_type),)+
        }

        #[async_trait]
        impl AMMFilter for PoolFilter {
            async fn filter(&self, amms: Vec<AMM>) -> Result<Vec<AMM>> {
                match self {
                    $(PoolFilter::$filter_type(filter) => filter.filter(amms).await,)+
                }
            }

            fn stage(&self) -> FilterStage {
                match self {
                    $(PoolFilter::$filter_type(filter) => filter.stage(),)+
                }
            }
        }

        $(
            impl From<$filter_type> for PoolFilter {
                fn from(filter: $filter_type) -> Self {
                    PoolFilter::$filter_type(filter)
                }
            }
        )+
    };
}

filter!(
    BlacklistFilter,
    PoolWhitelistFilter,
    TokenWhitelistFilter,
    // ValueFilter
);
