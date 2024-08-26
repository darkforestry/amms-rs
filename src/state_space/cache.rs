use super::StateChange;
use arraydeque::{ArrayDeque, CapacityError};

#[derive(Debug)]

pub struct StateChangeCache {
    // TODO: add comment explaining why we need to store the initial block number
    init_block: u64,
    cache: ArrayDeque<StateChange, 150>,
}

impl StateChangeCache {
    pub fn new(init_block: u64) -> Self {
        StateChangeCache {
            init_block,
            cache: ArrayDeque::new(),
        }
    }

    //TODO: push back

    pub fn push_front(
        &mut self,
        state_change: StateChange,
    ) -> Result<(), CapacityError<StateChange>> {
        self.0.push_front(state_change)
    }

    pub fn pop_front(&mut self) -> Option<StateChange> {
        self.cache.pop_front()
    }

    pub fn capacity(&self) -> usize {
        self.cache.capacity()
    }

    pub fn len(&self) -> usize {
        self.cache.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    pub fn clear(&mut self) {
        self.cache.clear();
    }

    // TODO: add state change to cache

    pub fn add_state_change_to_cache(
        &mut self,
        state_change: StateChange,
    ) -> Result<(), CapacityError<StateChange>> {
        let cache = &mut self.0;

        if cache.is_full() {
            cache.pop_back();
        }

        cache.push_front(state_change)
    }

    // TODO: update return type
    async fn unwind_state_changes(&mut self, block_to_unwind: u64) -> Vec<StateChange> {
        let cache = &mut self.cache;

        // TODO: check if the block to unwind is greater than the most recent state change block number
        // TODO: check if the block to unwind is less than the oldest state change block number,
        // NOTE: this needs to handle cases where cache is empty or the state changes are newer than the unwind
        // NOTE: we need to make sure we never silently end up in an incorrect state.

        if block_to_unwind < self.init_block {
            todo!("Handle error")
        }

        // We can unwrap here because we have checked that
        // the block number is < latest block and > the oldest block
        let pivot_idx = cache
            .iter()
            .rev()
            .position(|state_change| state_change.block_number < block_to_unwind);

        let state_changes = if let Some(pivot_idx) = pivot_idx {
            // Calculate the correct index from the front since we used `rev()`
            // NOTE: explain why we arent using len - 1 and index + 1
            // We need to drain the state changes including pivot + 1
            let drain_idx = cache.len() - pivot_idx;

            cache.drain(drain_idx..).collect()
        } else {
            cache.drain(0..).collect()
        };

        todo!("Need to flatten with rev precedence and then return the state changes")
    }
}
