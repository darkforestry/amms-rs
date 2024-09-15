use eyre::Result;

use crate::amms::amm::AMM;
use crate::state_space::filters::BlacklistFilter;
use crate::state_space::filters::WhitelistFilter;

pub trait AMMFilter {
    fn filter(&self, amms: Vec<AMM>) -> Result<Vec<AMM>>;
}

macro_rules! filter {
    ($($filter_type:ident),+ $(,)?) => {
        #[derive(Debug, Clone)]
        pub enum Filter {
            $($filter_type($filter_type),)+
        }

        impl AMMFilter for Filter {
            fn filter(&self, amms: Vec<AMM>) -> Result<Vec<AMM>> {
                match self {
                    $(Filter::$filter_type(filter) => filter.filter(amms),)+
                }
            }
        }
    };
}

filter!(BlacklistFilter, WhitelistFilter);
