use std::collections::HashMap;

use crate::amm::{AutomatedMarketMaker, AMM};

use super::StateChange;
use arraydeque::{ArrayDeque, CapacityError};

#[derive(Debug)]

pub struct StateChangeCache {
    oldest_block: u64,
    cache: ArrayDeque<StateChange, 150>,
}

impl StateChangeCache {
    pub fn new() -> Self {
        StateChangeCache {
            oldest_block: 0,
            cache: ArrayDeque::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    pub fn add_state_change_to_cache(
        &mut self,
        state_change: StateChange,
    ) -> Result<(), CapacityError<StateChange>> {
        let cache = &mut self.cache;

        if cache.is_full() {
            cache.pop_back();
            self.oldest_block = cache.back().unwrap().block_number;
        }

        cache.push_front(state_change)
    }

    /// Unwinds the state changes up to the given block number
    /// Returns the state of the affected AMMs at the block number provided
    pub fn unwind_state_changes(&mut self, block_to_unwind: u64) -> Vec<AMM> {
        let cache = &mut self.cache;

        if block_to_unwind < self.oldest_block {
            panic!("Block to unwind < oldest block in cache");
        }

        // If the block to unwind is greater than the latest state change in the block, exit early
        if cache
            .front()
            .map_or(true, |latest| block_to_unwind > latest.block_number)
        {
            return vec![];
        }

        let pivot_idx = cache
            .iter()
            .position(|state_change| state_change.block_number < block_to_unwind);

        let state_changes = if let Some(pivot_idx) = pivot_idx {
            cache.drain(..pivot_idx).collect()
        } else {
            cache.drain(..).collect::<Vec<StateChange>>()
        };

        self.flatten_state_changes(state_changes)
    }

    fn flatten_state_changes(&self, state_changes: Vec<StateChange>) -> Vec<AMM> {
        state_changes
            .into_iter()
            .rev()
            .fold(HashMap::new(), |mut amms, state_change| {
                for amm in state_change.state_change {
                    amms.entry(amm.address()).or_insert(amm);
                }
                amms
            })
            .into_values()
            .collect()
    }
}

// TODO: add tests
