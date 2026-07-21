#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeslotOwner {
    Brew,
    Cmce,
    /// One-slot packet-data bearer used by the opt-in SNDCP/WAP profile.
    Sndcp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeslotAllocErr {
    InvalidTimeslot(u8),
    InUse {
        ts: u8,
        owner: TimeslotOwner,
    },
    NotAllocated {
        ts: u8,
    },
    OwnerMismatch {
        ts: u8,
        owner: TimeslotOwner,
        actual: TimeslotOwner,
    },
}

/// Logical traffic timeslot allocator.
///
/// Historically the stack only had one carrier and therefore only TS2..=TS4 were
/// allocatable. For dual-carrier operation we keep the public "timeslot" value
/// as a compact logical bearer id:
///
/// - 2, 3, 4  => main carrier TS2, TS3, TS4
/// - 5, 6, 7  => secondary carrier TS2, TS3, TS4
///
/// Secondary-carrier TS1 is deliberately reserved for control/guard operation and
/// is not allocated as a traffic bearer.
///
/// That lets existing higher layers (Brew, Asterisk, EchoLink, CMCE call maps)
/// keep using `ts` as their bearer key without collisions when both carriers use
/// the same physical TETRA timeslot at the same time.
#[derive(Debug, Clone, Default)]
pub struct TimeslotAllocator {
    // Index 0 = logical TS2, 1 = TS3, 2 = TS4, 3 = TS5, 4 = TS6, 5 = TS7
    owners: [Option<TimeslotOwner>; 6],
}

impl TimeslotAllocator {
    pub const SINGLE_CARRIER_TRAFFIC_SLOTS: usize = 3;
    pub const DUAL_CARRIER_TRAFFIC_SLOTS: usize = 6;

    fn clamp_capacity(capacity: usize) -> usize {
        capacity.clamp(Self::SINGLE_CARRIER_TRAFFIC_SLOTS, Self::DUAL_CARRIER_TRAFFIC_SLOTS)
    }

    fn idx(ts: u8) -> Result<usize, TimeslotAllocErr> {
        if (2..=7).contains(&ts) {
            Ok((ts - 2) as usize)
        } else {
            Err(TimeslotAllocErr::InvalidTimeslot(ts))
        }
    }

    pub fn allocate_any(&mut self, owner: TimeslotOwner) -> Option<u8> {
        self.allocate_any_with_capacity(owner, Self::SINGLE_CARRIER_TRAFFIC_SLOTS)
    }

    pub fn allocate_any_with_capacity(&mut self, owner: TimeslotOwner, capacity: usize) -> Option<u8> {
        let capacity = Self::clamp_capacity(capacity);
        for (i, slot) in self.owners.iter_mut().take(capacity).enumerate() {
            if slot.is_none() {
                *slot = Some(owner);
                return Some(i as u8 + 2);
            }
        }
        None
    }

    pub fn reserve(&mut self, owner: TimeslotOwner, ts: u8) -> Result<(), TimeslotAllocErr> {
        self.reserve_with_capacity(owner, ts, Self::DUAL_CARRIER_TRAFFIC_SLOTS)
    }

    pub fn reserve_with_capacity(&mut self, owner: TimeslotOwner, ts: u8, capacity: usize) -> Result<(), TimeslotAllocErr> {
        let idx = Self::idx(ts)?;
        let capacity = Self::clamp_capacity(capacity);
        if idx >= capacity {
            return Err(TimeslotAllocErr::InvalidTimeslot(ts));
        }
        match self.owners[idx] {
            None => {
                self.owners[idx] = Some(owner);
                Ok(())
            }
            Some(existing) => Err(TimeslotAllocErr::InUse { ts, owner: existing }),
        }
    }

    pub fn release(&mut self, owner: TimeslotOwner, ts: u8) -> Result<(), TimeslotAllocErr> {
        let idx = Self::idx(ts)?;
        match self.owners[idx] {
            None => Err(TimeslotAllocErr::NotAllocated { ts }),
            Some(existing) if existing != owner => Err(TimeslotAllocErr::OwnerMismatch {
                ts,
                owner,
                actual: existing,
            }),
            Some(_) => {
                self.owners[idx] = None;
                Ok(())
            }
        }
    }

    pub fn owner(&self, ts: u8) -> Option<TimeslotOwner> {
        Self::idx(ts).ok().and_then(|idx| self.owners[idx])
    }

    pub fn is_free(&self, ts: u8) -> bool {
        self.owner(ts).is_none()
    }

    /// Number of currently unallocated single-carrier traffic bearers (logical TS2..=TS4).
    pub fn free_count(&self) -> usize {
        self.free_count_with_capacity(Self::SINGLE_CARRIER_TRAFFIC_SLOTS)
    }

    /// Number of currently unallocated traffic bearers for the configured carrier capacity.
    pub fn free_count_with_capacity(&self, capacity: usize) -> usize {
        let capacity = Self::clamp_capacity(capacity);
        self.owners.iter().take(capacity).filter(|o| o.is_none()).count()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sndcp_reservation_blocks_voice_until_release() {
        let mut alloc = TimeslotAllocator::default();
        alloc.reserve(TimeslotOwner::Sndcp, 2).unwrap();
        assert_eq!(
            alloc.reserve(TimeslotOwner::Cmce, 2),
            Err(TimeslotAllocErr::InUse { ts: 2, owner: TimeslotOwner::Sndcp })
        );
        alloc.release(TimeslotOwner::Sndcp, 2).unwrap();
        assert!(alloc.reserve(TimeslotOwner::Cmce, 2).is_ok());
    }
}
