use crate::amm::{AutomatedMarketMaker, AMM};

use alloy::{
    primitives::{Address, B256},
    rpc::types::eth::Filter,
};
use arraydeque::ArrayDeque;
use reth_exex::ExExNotification;
use reth_node_api::FullNodeComponents;
use reth_primitives::Log;
use reth_provider::BundleStateWithReceipts;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::sync::RwLock;

use super::{
    add_state_change_to_cache, error::StateChangeError, unwind_state_changes, StateChange,
    StateChangeCache, StateSpace,
};

#[derive(Debug)]
pub struct StateSpaceManagerExEx<Node>
where
    Node: FullNodeComponents,
{
    state: Arc<RwLock<StateSpace>>,
    _latest_synced_block: u64,
    state_change_cache: Arc<RwLock<StateChangeCache>>,
    _provider: Arc<Node::Provider>,
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
            _latest_synced_block: latest_synced_block,
            state_change_cache: Arc::new(RwLock::new(ArrayDeque::new())),
            _provider: provider,
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

    pub async fn process_notification(
        &self,
        notification: ExExNotification,
    ) -> Result<Vec<Address>, StateChangeError> {
        // TODO: return addresses affected by state changes
        match notification {
            ExExNotification::ChainCommitted { new } => {
                let bundled_state = new.state();
                self.handle_state_changes(bundled_state).await
            }

            ExExNotification::ChainReorged { old: _old, new } => {
                let bundled_state = new.state();
                self.handle_reorgs(bundled_state).await
            }
            ExExNotification::ChainReverted { old: _old } => Ok(vec![]),
        }
    }

    pub async fn handle_reorgs(
        &self,
        new: &BundleStateWithReceipts,
    ) -> Result<Vec<Address>, StateChangeError> {
        let block_number = new.first_block();

        let logs = (block_number..=(block_number + new.receipts().receipt_vec.len() as u64 - 1))
            .filter_map(|block_number| new.logs(block_number))
            .flatten()
            .cloned()
            .collect::<Vec<Log>>();
        // Unwind the state changes from the old state to the new state
        unwind_state_changes(
            self.state.clone(),
            self.state_change_cache.clone(),
            block_number,
        )
        .await?;
        self.modify_state_from_logs(logs, block_number).await
    }

    pub async fn handle_state_changes(
        &self,
        bundled_state: &BundleStateWithReceipts,
    ) -> Result<Vec<Address>, StateChangeError> {
        let block_number = bundled_state.first_block();

        let logs = (block_number
            ..=(block_number + bundled_state.receipts().receipt_vec.len() as u64 - 1))
            .filter_map(|block_number| bundled_state.logs(block_number))
            .flatten()
            .cloned()
            .collect::<Vec<Log>>();

        self.modify_state_from_logs(logs, block_number).await
    }

    async fn modify_state_from_logs(
        &self,
        logs: Vec<Log>,
        block_number: u64,
    ) -> Result<Vec<Address>, StateChangeError> {
        let mut updated_amms_set = HashSet::new();
        let mut updated_amms = vec![];
        let mut state_changes = vec![];

        for log in logs.into_iter() {
            // check if the log is from an amm in the state space
            if let Some(amm) = self.state.write().await.get_mut(&log.address) {
                if !updated_amms_set.contains(&log.address) {
                    updated_amms_set.insert(log.address);
                    updated_amms.push(log.address);
                }
                amm.sync_from_log(log)?;
                state_changes.push(amm.clone());
            }

            // Commit the [`StateChange`] to the cache at `block_number`
            if !state_changes.is_empty() {
                add_state_change_to_cache(
                    self.state_change_cache.clone(),
                    StateChange::new(Some(state_changes.clone()), block_number),
                )
                .await?;
            };
        }

        Ok(updated_amms)
    }
}
