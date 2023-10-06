use derive_builder::Builder;
use std::{collections::HashSet, str::FromStr, sync::Arc};

use ethers::{
    providers::Middleware,
    types::{Filter, U256},
};
use regex::Regex;
use spinoff::{spinners, Color, Spinner};

use crate::{
    amm::erc_4626::{ERC4626Vault, DEPOSIT_EVENT_SIGNATURE, WITHDRAW_EVENT_SIGNATURE},
    errors::AMMError,
};

lazy_static::lazy_static! {
    static ref HEX_REGEX: Regex = Regex::new(r"0x[0-9a-fA-F]+").expect("Could not compile regex");
}

/// ERC4626 vault discovery options
#[derive(Debug, Builder, Default)]
pub struct Erc4626DiscoveryOptions {
    /// From block number, if None then the discovery start block number will be 0
    #[builder(default = "0")]
    pub from_block: u64,
    /// To block number, if None then the discovery end block number will be the current block number
    #[builder(default)]
    pub to_block: Option<u64>,
    /// Block number step
    #[builder(default = "1000")]
    pub step: u64,
}

// Returns a vec of empty factories that match one of the Factory interfaces specified by each DiscoverableFactory
pub async fn discover_erc_4626_vaults<M: Middleware>(
    middleware: Arc<M>,
    options: Option<Erc4626DiscoveryOptions>,
) -> Result<Vec<ERC4626Vault>, AMMError<M>> {
    let spinner = Spinner::new(
        spinners::Dots,
        "Discovering new ERC 4626 vaults...",
        Color::Blue,
    );

    let block_filter =
        Filter::new().topic0(vec![DEPOSIT_EVENT_SIGNATURE, WITHDRAW_EVENT_SIGNATURE]);

    let options = options.unwrap_or_default();

    let mut from_block = options.from_block;
    let current_block = match options.to_block {
        Some(b) => b,
        None => middleware
            .get_block_number()
            .await
            .map_err(AMMError::MiddlewareError)?
            .as_u64(),
    };
    let step = options.step;

    let mut adheres_to_withdraw_event = HashSet::new();
    let mut adheres_to_deposit_event = HashSet::new();
    let mut identified_addresses = HashSet::new();

    //TODO: make this async
    while from_block < current_block {
        //Get pair created event logs within the block range
        let mut to_block = from_block + step - 1;
        if to_block > current_block {
            to_block = current_block;
        }

        let block_filter = block_filter.clone();
        //TODO: use a better method, this is just quick and scrappy
        let fallback_block_filter = block_filter.clone();

        let logs = match middleware
            .get_logs(&block_filter.from_block(from_block).to_block(to_block))
            .await
        {
            Ok(logs) => {
                from_block += step;
                logs
            }
            Err(err) => {
                let mut block_range = Vec::new();
                for m in HEX_REGEX.find_iter(&err.to_string()) {
                    let value = U256::from_str(m.as_str()).map_err(|_| AMMError::FromHexError)?;
                    block_range.push(value);
                }

                if block_range.is_empty() {
                    return Err(AMMError::MiddlewareError(err));
                } else {
                    let logs = middleware
                        .get_logs(
                            &fallback_block_filter
                                .from_block(block_range[0].as_u64())
                                .to_block(block_range[1].as_u64()),
                        )
                        .await
                        .map_err(AMMError::MiddlewareError)?;

                    from_block = block_range[1].as_u64();

                    logs
                }
            }
        };

        for log in logs {
            if log.topics[0] == DEPOSIT_EVENT_SIGNATURE {
                adheres_to_deposit_event.insert(log.address);
            } else if log.topics[0] == WITHDRAW_EVENT_SIGNATURE {
                adheres_to_withdraw_event.insert(log.address);
            }
        }
    }

    for address in adheres_to_deposit_event.iter() {
        if adheres_to_withdraw_event.contains(address) {
            identified_addresses.insert(address);
        }
    }

    let mut vaults = vec![];
    for identified_address in identified_addresses {
        //TODO: Add an interface check, but for now just try to get a new vault from address, if it fails then do not add it to the identified
        //TODO: vaults. This approach is inefficient but should work for now.

        if let Ok(vault) =
            ERC4626Vault::new_from_address(*identified_address, middleware.clone()).await
        {
            vaults.push(vault);
        }
    }

    spinner.success("All vaults discovered");
    Ok(vaults)
}
