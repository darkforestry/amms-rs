use super::StateChange;
use arraydeque::{ArrayDeque, CapacityError};

#[derive(Debug)]
pub struct StateChangeCache(ArrayDeque<StateChange, 150>);

impl StateChangeCache {
    pub fn new() -> Self {
        StateChangeCache(ArrayDeque::new())
    }

    //TODO: push back

    pub fn push_front(
        &mut self,
        state_change: StateChange,
    ) -> Result<(), CapacityError<StateChange>> {
        self.0.push_front(state_change)
    }

    pub fn pop_front(&mut self) -> Option<StateChange> {
        self.0.pop_front()
    }

    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn clear(&mut self) {
        self.0.clear();
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

    async fn unwind_state_changes(
        &mut self,
        block_to_unwind: u64,
    ) -> Result<(), CapacityError<StateChange>> {
    }
}
/// Unwinds the state changes cache for every block from the most recent state change cache back to the block to unwind -1.
// TODO: return vec of current state of amms after unwinding
async fn unwind_state_changes(block_to_unwind: u64) -> Result<(), StateChangeError> {
    let mut state_change_cache = state_change_cache.write().await;

    // TODO: iterate through state changes
    // TODO: check the block number, if it is >= block to unwind, keep going until we are < block to unwind and then, flatten and drain the updates

    loop {
        // check if the most recent state change block is >= the block to unwind,
        if let Some(state_change) = state_change_cache.get(0) {
            if state_change.block_number >= block_to_unwind {
                if let Some(option_state_changes) = state_change_cache.pop_front() {
                    if let Some(state_changes) = option_state_changes.state_change {
                        for amm_state in state_changes {
                            state.write().await.insert(amm_state.address(), amm_state);
                        }
                    }
                } else {
                    // We know that there is a state change from state_change_cache.get(0) so when we pop front without returning a value, there is an issue
                    return Err(StateChangeError::PopFrontError);
                }
            } else {
                return Ok(());
            }
        } else {
            // We return an error here because we never want to be unwinding past where we have state changes.
            // For example, if you initialize a state space that syncs to block 100, then immediately after there is a chain reorg to 95, we can not roll back the state
            // changes for an accurate state space. In this case, we return an error
            return Err(StateChangeError::NoStateChangesInCache);
        }
    }
}
