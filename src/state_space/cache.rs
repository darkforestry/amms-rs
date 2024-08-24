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

    // TODO: unwind state changes from cache
}
