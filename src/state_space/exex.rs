use crate::{
    amm::{AutomatedMarketMaker, AMM},
    errors::EventLogError,
};

use alloy::{
    network::Network,
    primitives::{Address, B256},
    rpc::types::eth::{Block, Filter},
    transports::Transport,
};
use arraydeque::ArrayDeque;
use futures::StreamExt;
use reth_exex::{ExExContext, ExExNotification};
use reth_node_api::{FullNodeComponents, FullNodeTypes};
use reth_primitives::{Log, Receipt, Receipts};
use reth_provider::BundleStateWithReceipts;
use std::{
    collections::{HashMap, HashSet},
    marker::PhantomData,
    sync::Arc,
};
use tokio::{
    sync::{
        mpsc::{Receiver, Sender},
        RwLock,
    },
    task::JoinHandle,
};

use super::{
    error::{StateChangeError, StateSpaceError},
    StateChangeCache, StateSpace,
};

#[derive(Debug)]
pub struct StateSpaceManagerExEx<Node>
where
    Node: FullNodeComponents,
{
    state: Arc<RwLock<StateSpace>>,
    latest_synced_block: u64,
    state_change_cache: Arc<RwLock<StateChangeCache>>,
    provider: Arc<Node::Provider>,
}

impl<N> StateSpaceManagerExEx<N>
where
    N: FullNodeComponents,
{
    pub fn new(amms: Vec<AMM>, latest_synced_block: u64, provider: Arc<N::Provider>) -> Self {
        let state: HashMap<Address, AMM> = amms
            .into_iter()
            .map(|amm| (amm.address(), amm))
            .collect::<HashMap<Address, AMM>>();

        Self {
            state: Arc::new(RwLock::new(state)),
            latest_synced_block,
            state_change_cache: Arc::new(RwLock::new(ArrayDeque::new())),
            provider,
        }
    }

    pub async fn filter(&self) -> Filter {
        let mut event_signatures: Vec<B256> = vec![];
        let mut amm_variants = HashSet::new();

        for amm in self.state.read().await.values() {
            let variant = match amm {
                AMM::UniswapV2Pool(_) => 0,
                AMM::UniswapV3Pool(_) => 1,
                AMM::ERC4626Vault(_) => 2,
            };

            if !amm_variants.contains(&variant) {
                amm_variants.insert(variant);
                event_signatures.extend(amm.sync_on_event_signatures());
            }
        }

        // Create a new filter
        Filter::new().event_signature(event_signatures)
    }

    pub async fn process_notification(&self, notification: ExExNotification) {
        // TODO: return addresses affected by state changes
        match notification {
            ExExNotification::ChainCommitted { new } => {
                let bundled_state = new.state();

                self.handle_state_changes(
                    self.state.clone(),
                    self.state_change_cache.clone(),
                    bundled_state,
                );
            }

            ExExNotification::ChainReorged { old, new } => {}
            ExExNotification::ChainReverted { old } => {}
        }
    }

    pub async fn handle_state_changes(
        &self,
        state: Arc<RwLock<StateSpace>>,
        state_change_cache: Arc<RwLock<StateChangeCache>>,
        bundled_state: &BundleStateWithReceipts,
    ) -> Result<Vec<Address>, StateChangeError> {
        let mut updated_amms_set = HashSet::new();
        let mut updated_amms = vec![];
        let mut state_changes = vec![];

        // TODO: need to collect block number with logs by getting the first block number from the bundled state
        // TODO: then we can extract logs

        let receipts = receipts
            .iter()
            .flat_map(|inner_vec| {
                inner_vec
                    .iter()
                    .filter_map(|opt_receipt| opt_receipt.as_ref())
            })
            .collect::<Vec<&Receipt>>();

        //------------------------------------------------------------

        // let mut last_log_block_number = if let Some(log) = logs.first() {
        //     get_block_number_from_log(log)?
        // } else {
        //     return Ok(updated_amms);
        // };

        // for log in logs.into_iter() {
        //     let log_block_number = get_block_number_from_log(&log)?;

        //     // check if the log is from an amm in the state space
        //     if let Some(amm) = state.write().await.get_mut(&log.address()) {
        //         if !updated_amms_set.contains(&log.address()) {
        //             updated_amms_set.insert(log.address());
        //             updated_amms.push(log.address());
        //         }

        //         state_changes.push(amm.clone());
        //         amm.sync_from_log(log)?;
        //     }

        //     // Commit state changes if the block has changed since last log
        //     if log_block_number != last_log_block_number {
        //         if state_changes.is_empty() {
        //             add_state_change_to_cache(
        //                 state_change_cache.clone(),
        //                 StateChange::new(None, last_log_block_number),
        //             )
        //             .await?;
        //         } else {
        //             add_state_change_to_cache(
        //                 state_change_cache.clone(),
        //                 StateChange::new(Some(state_changes), last_log_block_number),
        //             )
        //             .await?;
        //             state_changes = vec![];
        //         };

        //         last_log_block_number = log_block_number;
        //     }
        // }

        // if state_changes.is_empty() {
        //     add_state_change_to_cache(
        //         state_change_cache,
        //         StateChange::new(None, last_log_block_number),
        //     )
        //     .await?;
        // } else {
        //     add_state_change_to_cache(
        //         state_change_cache,
        //         StateChange::new(Some(state_changes), last_log_block_number),
        //     )
        //     .await?;
        // };

        Ok(updated_amms)
    }
}
