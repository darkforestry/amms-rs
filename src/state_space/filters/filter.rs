use crate::amms::amm::AMM;
use crate::state_space::filters::BlacklistFilter;
use crate::state_space::filters::WhitelistFilter;

pub trait AmmFilter {
    fn filter(&self, amms: Vec<AMM>) -> Vec<AMM>;
}

macro_rules! filter {
    ($($filter_type:ident),+ $(,)?) => {
        #[derive(Debug, Clone)]
        pub enum Filter {
            $($filter_type($filter_type),)+
        }

        impl AmmFilter for Filter {
            fn filter(&self, amms: Vec<AMM>) -> Vec<AMM> {
                match self {
                    $(Filter::$filter_type(filter) => filter.filter(amms),)+
                }
            }
        }
    };
}

filter!(BlacklistFilter, WhitelistFilter);
