use async_trait::async_trait;
use eyre::Result;

use crate::amms::amm::AMM;
use crate::state_space::filters::BlacklistFilter;
use crate::state_space::filters::WhitelistFilter;

#[async_trait]
pub trait AMMFilter: Into<PoolFilter> {
    async fn filter(&self, amms: Vec<AMM>) -> Result<Vec<AMM>>;
    fn stage(&self) -> FilterStage;
}

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
    };
}

filter!(BlacklistFilter, WhitelistFilter);
