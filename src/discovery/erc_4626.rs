use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
    sync::Arc,
    time::{self, Duration},
};

use ethers::{
    providers::Middleware,
    types::{Filter, H160, U256},
};
use regex::Regex;
use spinoff::{spinners, Color, Spinner};

use crate::{
    amm::erc_4626::{ERC4626Vault, DEPOSIT_EVENT_SIGNATURE, WITHDRAW_EVENT_SIGNATURE},
    errors::DAMMError,
};

// Returns a vec of empty factories that match one of the Factory interfaces specified by each DiscoverableFactory
pub async fn discover_erc_4626_vaults<M: Middleware>(
    middleware: Arc<M>,
) -> Result<Vec<ERC4626Vault>, DAMMError<M>> {
    let spinner = Spinner::new(
        spinners::Dots,
        "Discovering new ERC 4626 vaults...",
        Color::Blue,
    );

    let block_filter =
        Filter::new().topic0(vec![DEPOSIT_EVENT_SIGNATURE, WITHDRAW_EVENT_SIGNATURE]);

    let current_block = middleware
        .get_block_number()
        .await
        .map_err(DAMMError::MiddlewareError)?
        .as_u64();

    //For each block within the range, get all pairs asynchronously
    let step = 100000;

    let mut identified_addresses = HashSet::new();

    let mut from_block = 0;
    while from_block < current_block {
        //Get pair created event logs within the block range
        let mut to_block = from_block + step as u64;
        if to_block > current_block {
            to_block = current_block;
        }

        dbg!(from_block, to_block);

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
                let hex_pattern = Regex::new(r"0x[0-9a-fA-F]+").unwrap();
                let block_range = hex_pattern
                    .find_iter(&err.to_string())
                    .map(|m| {
                        U256::from_str(m.as_str()).expect("Could not convert hex number to U256")
                    })
                    .collect::<Vec<U256>>();

                if block_range.is_empty() {
                    return Err(DAMMError::MiddlewareError(err));
                } else {
                    let logs = middleware
                        .get_logs(
                            &fallback_block_filter
                                .from_block(block_range[0].as_u64())
                                .to_block(block_range[1].as_u64()),
                        )
                        .await
                        .map_err(DAMMError::MiddlewareError)?;

                    from_block = block_range[1].as_u64();

                    logs
                }
            }
        };

        for log in logs {
            //TODO: Add an interface check, but for now just try to get a new vault from address, if it fails then do not add it to the identified
            //TODO: vaults. This approach is inefficient but should work for now.

            if let None = identified_addresses.get(&log.address) {
                identified_addresses.insert(log.address);
            }
        }

        std::thread::sleep(Duration::from_millis(50));
    }

    let mut vaults = vec![];

    for identified_address in identified_addresses {
        std::thread::sleep(Duration::from_millis(100));
        if let Ok(vault) =
            ERC4626Vault::new_from_address(identified_address, middleware.clone()).await
        {
            dbg!(vault);
            vaults.push(vault);
        } else {
            dbg!("not a vault", identified_address);
        }
    }

    spinner.success("All vaults discovered");
    Ok(vaults)
}
