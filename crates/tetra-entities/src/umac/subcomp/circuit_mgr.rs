use std::collections::VecDeque;

use tetra_core::Direction;
use tetra_saps::control::call_control::Circuit;

pub struct CircuitMgr {
    pub dl: [Option<Circuit>; 4],
    pub ul: [Option<Circuit>; 4],

    /// Data blocks queued to be transmitted, per timeslot
    pub tx_data: [VecDeque<Vec<u8>>; 4],
}

impl CircuitMgr {
    pub fn new() -> Self {
        Self {
            dl: [None, None, None, None],
            ul: [None, None, None, None],
            tx_data: [VecDeque::new(), VecDeque::new(), VecDeque::new(), VecDeque::new()],
        }
    }

    fn ts_index(ts: u8) -> Option<usize> {
        if (1..=4).contains(&ts) {
            Some(ts as usize - 1)
        } else {
            None
        }
    }

    pub fn is_active(&self, dir: Direction, ts: u8) -> bool {
        let Some(idx) = Self::ts_index(ts) else {
            tracing::warn!(
                "UMAC CircuitMgr: invalid physical timeslot {} for is_active({:?}); ignoring",
                ts,
                dir
            );
            return false;
        };

        match dir {
            Direction::Dl => self.dl[idx].is_some(),
            Direction::Ul => self.ul[idx].is_some(),
            _ => {
                tracing::error!("UMAC CircuitMgr: called with non-specific direction {:?}", dir);
                false
            }
        }
    }

    pub fn get_usage(&self, dir: Direction, ts: u8) -> Option<u8> {
        let Some(idx) = Self::ts_index(ts) else {
            tracing::warn!(
                "UMAC CircuitMgr: invalid physical timeslot {} for get_usage({:?}); ignoring",
                ts,
                dir
            );
            return None;
        };

        match dir {
            Direction::Dl => self.dl[idx].as_ref().map(|circuit| circuit.usage),
            Direction::Ul => self.ul[idx].as_ref().map(|circuit| circuit.usage),
            _ => {
                tracing::error!("UMAC CircuitMgr: called with non-specific direction {:?}", dir);
                None
            }
        }
    }

    /// Closes an active circuit, and return the Circuit to the caller
    pub fn close_circuit(&mut self, dir: Direction, ts: u8) -> Option<Circuit> {
        let Some(idx) = Self::ts_index(ts) else {
            tracing::warn!(
                "UMAC CircuitMgr: invalid physical timeslot {} for close_circuit({:?}); ignoring",
                ts,
                dir
            );
            return None;
        };

        match dir {
            Direction::Dl => {
                self.tx_data[idx].clear();
                self.dl[idx].take()
            }
            Direction::Ul => self.ul[idx].take(),
            _ => {
                tracing::error!("UMAC CircuitMgr: called with non-specific direction {:?}", dir);
                None
            }
        }
    }

    /// Creates a new circuit on the given direction and timeslot.
    ///
    /// The UMAC scheduler is per carrier. Therefore this low-level manager only accepts
    /// physical air-interface timeslots 1..=4. Higher layers may use logical TS5..TS7
    /// for secondary-carrier traffic, but those must be mapped back to physical TS2..TS4
    /// before reaching this component.
    pub fn create_circuit(&mut self, dir: Direction, circuit: Circuit) {
        let ts = circuit.ts;
        let Some(idx) = Self::ts_index(ts) else {
            tracing::warn!(
                "UMAC CircuitMgr: refusing to create {:?} circuit on invalid physical timeslot {}",
                dir,
                ts
            );
            return;
        };

        // Sanity check
        if self.is_active(dir, ts) {
            tracing::warn!("CircuitMgr::create had still active circuit on {:?} {}", dir, ts);
            self.close_circuit(dir, ts);
        }

        match dir {
            Direction::Dl => {
                if !self.tx_data[idx].is_empty() {
                    tracing::warn!("CircuitMgr::create had pending tx_data on Dl {}", ts);
                    self.tx_data[idx].clear();
                }
                self.dl[idx] = Some(circuit);
            }
            Direction::Ul => self.ul[idx] = Some(circuit),
            _ => {
                tracing::error!("UMAC CircuitMgr: called with non-specific direction {:?}", dir);
            }
        }
    }

    /// Put a block in the queue for transmission on an associated channel
    pub fn put_block(&mut self, ts: u8, block: Vec<u8>) {
        let Some(idx) = Self::ts_index(ts) else {
            tracing::warn!(
                "UMAC CircuitMgr: refusing put_block on invalid physical timeslot {}",
                ts
            );
            return;
        };

        if !self.is_active(Direction::Dl, ts) {
            tracing::warn!("CircuitMgr::put_block on inactive circuit {:?} {}", Direction::Dl, ts);
            return;
        }
        self.tx_data[idx].push_back(block);
    }

    /// Take a to-be-transmitted block from the queue
    pub fn take_block(&mut self, ts: u8) -> Option<Vec<u8>> {
        let Some(idx) = Self::ts_index(ts) else {
            tracing::warn!(
                "UMAC CircuitMgr: refusing take_block on invalid physical timeslot {}",
                ts
            );
            return None;
        };

        if !self.is_active(Direction::Dl, ts) {
            tracing::warn!("CircuitMgr::take_block on inactive circuit {:?} {}", Direction::Dl, ts);
            return None;
        }
        self.tx_data[idx].pop_front()
    }
}
