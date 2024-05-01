use std::{collections::HashSet, str::FromStr, sync::Arc};

use alloy::{
    network::Network, primitives::U256, providers::Provider, rpc::types::eth::Filter,
    sol_types::SolEvent, transports::Transport,
};
use regex::Regex;

use crate::{
    amm::erc_4626::{ERC4626Vault, IERC4626Vault},
    errors::AMMError,
};

lazy_static::lazy_static! {
    static ref HEX_REGEX: Regex = Regex::new(r"0x[0-9a-fA-F]+").expect("Could not compile regex");
}

// Returns a vec of empty factories that match one of the Factory interfaces specified by each DiscoverableFactory
pub async fn discover_erc_4626_vaults<T, N, P>(
    provider: Arc<P>,
    step: u64,
) -> Result<Vec<ERC4626Vault>, AMMError>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N>,
{
    let event_signatures = vec![
        IERC4626Vault::Deposit::SIGNATURE_HASH,
        IERC4626Vault::Withdraw::SIGNATURE_HASH,
    ];
    let block_filter = Filter::new().event_signature(event_signatures.clone());
    tracing::trace!(?event_signatures);

    let current_block = provider.get_block_number().await?;

    let mut adheres_to_withdraw_event = HashSet::new();
    let mut adheres_to_deposit_event = HashSet::new();
    let mut identified_addresses = HashSet::new();

    let mut from_block = 0;
    // TODO: make this async
    while from_block < current_block {
        // Get pair created event logs within the block range
        let mut to_block = from_block + step - 1;
        if to_block > current_block {
            to_block = current_block;
        }

        let block_filter = block_filter.clone();
        // TODO: use a better method, this is just quick and scrappy
        let fallback_block_filter = block_filter.clone();

        let logs = match provider
            .get_logs(&block_filter.from_block(from_block).to_block(to_block))
            .await
        {
            Ok(logs) => {
                from_block += step;
                logs
            }
            Err(err) => {
                tracing::warn!("error getting logs");

                let mut block_range = Vec::new();
                for m in HEX_REGEX.find_iter(&err.to_string()) {
                    let value = U256::from_str(m.as_str()).map_err(|_| AMMError::FromHexError)?;
                    block_range.push(value);
                }

                if block_range.is_empty() {
                    return Err(AMMError::TransportError(err));
                } else {
                    tracing::warn!(
                        "getting logs from blocks {}-{} instead",
                        block_range[0],
                        block_range[1]
                    );
                    let logs = provider
                        .get_logs(
                            &fallback_block_filter
                                .from_block(block_range[0].to::<u64>())
                                .to_block(block_range[1].to::<u64>()),
                        )
                        .await?;

                    from_block = block_range[1].to::<u64>();

                    logs
                }
            }
        };

        for log in logs {
            if log.topics()[0] == IERC4626Vault::Deposit::SIGNATURE_HASH {
                adheres_to_deposit_event.insert(log.address());
            } else if log.topics()[0] == IERC4626Vault::Withdraw::SIGNATURE_HASH {
                adheres_to_withdraw_event.insert(log.address());
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
            ERC4626Vault::new_from_address(*identified_address, provider.clone()).await
        {
            vaults.push(vault);
        }
    }

    Ok(vaults)
}
