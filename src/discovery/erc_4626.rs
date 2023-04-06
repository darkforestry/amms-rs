use std::{collections::HashMap, sync::Arc};

use ethers::{
    providers::Middleware,
    types::{Filter, H160},
};
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

    let from_block = 0;
    let current_block = middleware
        .get_block_number()
        .await
        .map_err(DAMMError::MiddlewareError)?
        .as_u64();

    //For each block within the range, get all pairs asynchronously
    let step = 100000;

    let mut identified_vaults = HashMap::new();

    for from_block in (from_block..=current_block).step_by(step) {
        //Get pair created event logs within the block range
        let mut to_block = from_block + step as u64;
        if to_block > current_block {
            to_block = current_block;
        }

        let block_filter = block_filter.clone();
        let logs = middleware
            .get_logs(&block_filter.from_block(from_block).to_block(to_block))
            .await
            .map_err(DAMMError::MiddlewareError)?;

        for log in logs {
            //TODO: interface check

            identified_vaults.insert(
                log.address,
                ERC4626Vault::new_from_address(log.address, middleware.clone()).await?,
            );
        }
    }

    spinner.success("All factories discovered");
    Ok(identified_vaults
        .iter()
        .map(|(_, vault)| *vault)
        .collect::<Vec<ERC4626Vault>>())
}
