//! SoA (Structure of Arrays) match store.
//!
//! - All match states stored in a contiguous `Vec` for cache efficiency.
//! - DashMap maps `(match_id, token)` to array index.
//! - Free list recycles indices of completed matches.

use protocol::MatchState;

/// SoA match store.
///
/// In production this will use `DashMap` for concurrent access.
/// For now, a simplified single-threaded version for prototyping.
#[derive(Debug)]
pub struct MatchStore {
    /// Contiguous array of match states.
    states: Vec<Option<MatchState>>,
    /// Free list of recyclable indices.
    free_list: Vec<u32>,
    /// Total matches created (for generation counter).
    total_created: u64,
}

impl MatchStore {
    /// Create a new empty match store with pre-allocated capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            states: Vec::with_capacity(capacity),
            free_list: Vec::new(),
            total_created: 0,
        }
    }

    /// Insert a new match into the store. Returns the index.
    pub fn insert(&mut self, mut state: MatchState) -> u32 {
        // Set RNG seed deterministically from match_id + token
        state.rng_seed = self.total_created;
        self.total_created += 1;

        if let Some(free) = self.free_list.pop() {
            self.states[free as usize] = Some(state);
            free
        } else {
            let idx = self.states.len() as u32;
            self.states.push(Some(state));
            idx
        }
    }

    /// Get a mutable reference to a match state by index.
    pub fn get_mut(&mut self, index: u32) -> Option<&mut MatchState> {
        self.states.get_mut(index as usize)?.as_mut()
    }

    /// Get a shared reference to a match state by index.
    pub fn get(&self, index: u32) -> Option<&MatchState> {
        self.states.get(index as usize)?.as_ref()
    }

    /// Remove a match and recycle its index.
    pub fn remove(&mut self, index: u32) {
        if let Some(slot) = self.states.get_mut(index as usize) {
            if slot.is_some() {
                *slot = None;
                self.free_list.push(index);
            }
        }
    }

    /// Number of active (non-removed) matches.
    pub fn active_count(&self) -> usize {
        self.states.len() - self.free_list.len()
    }

    /// Total capacity (including recycled slots).
    pub fn capacity(&self) -> usize {
        self.states.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::{MatchState, TacticState};

    fn make_test_match(id: u32) -> MatchState {
        MatchState {
            match_id: id,
            token: [0u8; 16],
            last_seq: 0,
            score: [0, 0],
            minute: 0,
            possession: 0.5,
            stamina: [1.0, 1.0],
            tactic: [TacticState::default(), TacticState::default()],
            rng_seed: 0,
        }
    }

    #[test]
    fn test_insert_and_get() {
        let mut store = MatchStore::with_capacity(10);
        let idx = store.insert(make_test_match(1));
        assert_eq!(store.get(idx).unwrap().match_id, 1);
        assert_eq!(store.active_count(), 1);
    }

    #[test]
    fn test_remove_and_recycle() {
        let mut store = MatchStore::with_capacity(10);
        let idx1 = store.insert(make_test_match(1));
        let _idx2 = store.insert(make_test_match(2));
        assert_eq!(store.active_count(), 2);

        store.remove(idx1);
        assert_eq!(store.active_count(), 1);
        assert!(store.get(idx1).is_none());

        // Inserting again should recycle idx1
        let idx3 = store.insert(make_test_match(3));
        assert_eq!(idx3, idx1); // recycled
        assert_eq!(store.active_count(), 2);
    }
}
