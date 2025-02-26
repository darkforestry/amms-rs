use std::collections::HashMap;

use crate::amms::amm::{AutomatedMarketMaker, AMM};
use arraydeque::ArrayDeque;
use serde::{Deserialize, Serialize};

#[derive(Debug)]

pub struct StateChangeCache<const CAP: usize> {
    oldest_block: u64,
    cache: ArrayDeque<StateChange, CAP>,
}

impl<const CAP: usize> Default for StateChangeCache<CAP> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const CAP: usize> StateChangeCache<CAP> {
    pub fn new() -> Self {
        StateChangeCache {
            oldest_block: 0,
            cache: ArrayDeque::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    pub fn push(&mut self, state_change: StateChange) {
        let cache = &mut self.cache;

        if cache.is_full() {
            cache.pop_back();
            self.oldest_block = cache.back().unwrap().block_number;
        }

        // We can unwrap here since we check if the cache is full
        cache.push_front(state_change).unwrap();
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
            .is_none_or(|latest| block_to_unwind > latest.block_number)
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

// NOTE: we can probably make this more efficient and create a state change struct for each amm rather than
// cloning each amm when caching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChange {
    pub state_change: Vec<AMM>,
    pub block_number: u64,
}

impl StateChange {
    pub fn new(state_change: Vec<AMM>, block_number: u64) -> Self {
        Self {
            block_number,
            state_change,
        }
    }
}
// TODO: add tests
