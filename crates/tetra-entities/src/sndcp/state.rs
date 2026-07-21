//! Runtime state for the SwMI side of ETSI EN 300 392-2 clause 28 SNDCP.
//!
//! The implementation deliberately models the capability profile advertised by
//! NetCore-Tetra: IPv4, no SNDCP compression, one basic/advanced-link PDCH and
//! multiple primary and secondary PDP contexts/NSAPIs per subscriber.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use super::qos::QosProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContextKey {
    pub issi: u32,
    pub nsapi: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdpState {
    Standby,
    Ready,
    /// The subscriber is globally READY, but this PDP context's CONTEXT_READY
    /// timer has expired and it must be re-announced before new traffic.
    Quiescent,
    Suspended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextAvailability {
    Available,
    ScheduleSuspended,
    Reserved(u8),
}

impl ContextAvailability {
    pub fn from_code(code: u8) -> Self {
        match code & 7 {
            0 => Self::Available,
            1 => Self::ScheduleSuspended,
            other => Self::Reserved(other),
        }
    }

    pub fn code(self) -> u8 {
        match self {
            Self::Available => 0,
            Self::ScheduleSuspended => 1,
            Self::Reserved(code) => code & 7,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextUsage {
    /// Internal runtime state: the context is not paused by SN-MODIFY USAGE.
    Active,
    SchedulePaused,
    ContextPaused,
    Reserved(u8),
}

impl ContextUsage {
    pub fn from_code(code: u8) -> Self {
        match code & 7 {
            0 => Self::SchedulePaused,
            1 => Self::ContextPaused,
            other => Self::Reserved(other),
        }
    }

    pub fn code(self) -> Option<u8> {
        match self {
            Self::Active => None,
            Self::SchedulePaused => Some(0),
            Self::ContextPaused => Some(1),
            Self::Reserved(code) => Some(code & 7),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PdpContext {
    pub address: [u8; 4],
    pub state: PdpState,
    pub availability: ContextAvailability,
    pub usage: ContextUsage,
    pub pdu_priority_max: u8,
    pub requested_pcomp: u8,
    pub requested_dcomp: u8,
    pub mtu_octets: usize,
    pub network_endpoint_id: Option<u16>,
    pub primary_nsapi: Option<u8>,
    pub packet_data_ms_type: u8,
    pub qos: QosProfile,
    pub created_at: Instant,
    pub last_activity: Instant,
    pub ready_deadline: Option<Instant>,
    pub standby_deadline: Option<Instant>,
}

impl PdpContext {
    pub fn new(
        address: [u8; 4],
        pdu_priority_max: u8,
        mtu_octets: usize,
        standby_timer_code: u8,
        now: Instant,
    ) -> Self {
        Self {
            address,
            state: PdpState::Standby,
            availability: ContextAvailability::Available,
            usage: ContextUsage::Active,
            pdu_priority_max: pdu_priority_max.min(7),
            requested_pcomp: 0,
            requested_dcomp: 0,
            mtu_octets,
            network_endpoint_id: None,
            primary_nsapi: None,
            packet_data_ms_type: 0,
            qos: QosProfile::Background,
            created_at: now,
            last_activity: now,
            ready_deadline: None,
            standby_deadline: deadline(now, standby_timer_duration(standby_timer_code)),
        }
    }

    pub fn enter_ready(&mut self, ready_timer_code: u8, now: Instant) {
        self.state = PdpState::Ready;
        self.usage = ContextUsage::Active;
        self.last_activity = now;
        self.ready_deadline = deadline(now, context_ready_timer_duration(self.qos.context_ready_timer(), ready_timer_code));
        self.standby_deadline = None;
    }

    pub fn refresh_ready(&mut self, ready_timer_code: u8, now: Instant) {
        if self.state == PdpState::Ready {
            self.last_activity = now;
            self.ready_deadline = deadline(now, context_ready_timer_duration(self.qos.context_ready_timer(), ready_timer_code));
        }
    }

    pub fn enter_standby(&mut self, standby_timer_code: u8, now: Instant) {
        self.state = PdpState::Standby;
        self.usage = ContextUsage::Active;
        self.last_activity = now;
        self.ready_deadline = None;
        self.standby_deadline = deadline(now, standby_timer_duration(standby_timer_code));
    }

    pub fn suspend(&mut self, standby_timer_code: u8, now: Instant) {
        self.state = PdpState::Suspended;
        self.usage = ContextUsage::ContextPaused;
        self.last_activity = now;
        self.ready_deadline = None;
        self.standby_deadline = deadline(now, standby_timer_duration(standby_timer_code));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerEvent {
    ReadyExpired(u32),
    ContextReadyExpired(ContextKey),
    StandbyExpired(ContextKey),
}

#[derive(Debug, Default)]
pub struct ContextTable {
    contexts: HashMap<ContextKey, PdpContext>,
    default_priorities: HashMap<u32, u8>,
    network_endpoint_ids: HashMap<u32, u16>,
    bearer_owner: Option<u32>,
    bearer_nsapis: HashSet<u8>,
    bearer_ready_deadline: Option<Instant>,
}

impl ContextTable {
    pub fn len(&self) -> usize {
        self.contexts.len()
    }

    pub fn contexts_for_issi(&self, issi: u32) -> usize {
        self.contexts.keys().filter(|key| key.issi == issi).count()
    }


    pub fn ensure_network_endpoint_id(&mut self, issi: u32) -> u16 {
        if let Some(value) = self.network_endpoint_ids.get(&issi).copied() {
            return value;
        }
        let used = self.network_endpoint_ids.values().copied().collect::<HashSet<_>>();
        let seed = ((issi ^ (issi >> 16)) as u16).max(1);
        let value = (0u16..=u16::MAX)
            .map(|offset| seed.wrapping_add(offset))
            .find(|candidate| *candidate != 0 && !used.contains(candidate))
            .unwrap_or(1);
        self.network_endpoint_ids.insert(issi, value);
        value
    }

    pub fn network_endpoint_id(&self, issi: u32) -> Option<u16> {
        self.network_endpoint_ids.get(&issi).copied()
    }

    pub fn update_ms_type(&mut self, issi: u32, packet_data_ms_type: u8) {
        for (key, context) in &mut self.contexts {
            if key.issi == issi {
                context.packet_data_ms_type = packet_data_ms_type;
            }
        }
    }

    pub fn update_secondary_addresses(&mut self, issi: u32, primary_nsapi: u8, address: [u8; 4]) {
        for (key, context) in &mut self.contexts {
            if key.issi == issi && context.primary_nsapi == Some(primary_nsapi) {
                context.address = address;
            }
        }
    }

    pub fn family_nsapis(&self, issi: u32, nsapi: u8) -> Vec<u8> {
        let is_primary = self
            .contexts
            .get(&ContextKey { issi, nsapi })
            .is_some_and(|context| context.primary_nsapi.is_none());
        let mut nsapis = vec![nsapi];
        if is_primary {
            nsapis.extend(
                self.contexts
                    .iter()
                    .filter_map(|(key, context)| {
                        (key.issi == issi && context.primary_nsapi == Some(nsapi)).then_some(key.nsapi)
                    }),
            );
        }
        nsapis.sort_unstable();
        nsapis.dedup();
        nsapis
    }

    pub fn get(&self, key: ContextKey) -> Option<&PdpContext> {
        self.contexts.get(&key)
    }

    pub fn get_mut(&mut self, key: ContextKey) -> Option<&mut PdpContext> {
        self.contexts.get_mut(&key)
    }

    pub fn insert(&mut self, key: ContextKey, context: PdpContext) -> Option<PdpContext> {
        self.contexts.insert(key, context)
    }

    pub fn remove(&mut self, key: ContextKey) -> Option<PdpContext> {
        if self.bearer_owner == Some(key.issi) {
            self.bearer_nsapis.remove(&key.nsapi);
        }
        let removed = self.contexts.remove(&key);
        self.clear_bearer_if_empty();
        self.cleanup_subscriber_if_unused(key.issi);
        removed
    }

    pub fn remove_all_for_issi(&mut self, issi: u32) -> Vec<(ContextKey, PdpContext)> {
        let keys = self
            .contexts
            .keys()
            .filter(|key| key.issi == issi)
            .copied()
            .collect::<Vec<_>>();
        let mut removed = Vec::with_capacity(keys.len());
        for key in keys {
            if let Some(context) = self.contexts.remove(&key) {
                removed.push((key, context));
            }
        }
        if self.bearer_owner == Some(issi) {
            self.bearer_owner = None;
            self.bearer_nsapis.clear();
            self.bearer_ready_deadline = None;
        }
        self.default_priorities.remove(&issi);
        self.network_endpoint_ids.remove(&issi);
        removed
    }

    pub fn addresses(&self) -> impl Iterator<Item = [u8; 4]> + '_ {
        self.contexts.values().map(|context| context.address)
    }

    pub fn address_in_use_by_other(&self, key: ContextKey, address: [u8; 4]) -> bool {
        self.contexts.iter().any(|(other_key, context)| {
            if *other_key == key || context.address != address {
                return false;
            }
            // A secondary PDP context is required to share its primary context's
            // address. It must therefore not make a primary reactivation look like
            // a collision with another subscriber/context family.
            !(other_key.issi == key.issi && context.primary_nsapi == Some(key.nsapi))
        })
    }

    pub fn bearer_owner(&self) -> Option<u32> {
        self.bearer_owner
    }

    pub fn refresh_bearer_ready(&mut self, issi: u32, ready_timer_code: u8, now: Instant) -> bool {
        if self.bearer_owner != Some(issi) {
            return false;
        }
        self.bearer_ready_deadline = deadline(now, ready_timer_duration(ready_timer_code));
        true
    }

    pub fn bearer_ready_deadline(&self) -> Option<Instant> {
        self.bearer_ready_deadline
    }

    pub fn can_claim_bearer(&self, issi: u32) -> bool {
        self.bearer_owner.is_none() || self.bearer_owner == Some(issi)
    }

    /// Record a successful MAC reservation. Multiple NSAPIs belonging to the same
    /// ISSI may share the single PDCH; another ISSI must wait until it is released.
    pub fn claim_bearer(&mut self, issi: u32, nsapis: &[u8]) -> bool {
        if !self.can_claim_bearer(issi) {
            return false;
        }
        if self.bearer_owner != Some(issi) {
            self.bearer_ready_deadline = None;
        }
        self.bearer_owner = Some(issi);
        self.bearer_nsapis.extend(nsapis.iter().copied().filter(|nsapi| (1..=14).contains(nsapi)));
        true
    }

    pub fn release_bearer_nsapis(&mut self, issi: u32, nsapis: &[u8]) -> bool {
        if self.bearer_owner != Some(issi) {
            return false;
        }
        for nsapi in nsapis {
            self.bearer_nsapis.remove(nsapi);
        }
        self.clear_bearer_if_empty()
    }

    pub fn release_bearer_for_issi(&mut self, issi: u32) -> bool {
        if self.bearer_owner != Some(issi) {
            return false;
        }
        self.bearer_owner = None;
        self.bearer_nsapis.clear();
        self.bearer_ready_deadline = None;
        true
    }

    fn clear_bearer_if_empty(&mut self) -> bool {
        if self.bearer_owner.is_some() && self.bearer_nsapis.is_empty() {
            self.bearer_owner = None;
            self.bearer_ready_deadline = None;
            true
        } else {
            false
        }
    }

    pub fn bearer_nsapis(&self) -> impl Iterator<Item = u8> + '_ {
        self.bearer_nsapis.iter().copied()
    }

    pub fn set_default_priority(&mut self, issi: u32, priority: u8) {
        self.default_priorities.insert(issi, priority.min(7));
    }

    pub fn track_network_default_priority(&mut self, issi: u32) {
        self.default_priorities.remove(&issi);
    }

    pub fn default_priority(&self, issi: u32, network_default: u8) -> u8 {
        self.default_priorities.get(&issi).copied().unwrap_or(network_default.min(7))
    }

    pub fn tick(&mut self, now: Instant, standby_timer_code: u8) -> Vec<TimerEvent> {
        let mut events = Vec::new();
        let mut standby_expired_issis = HashSet::new();

        // READY is a subscriber/bearer state, not a per-NSAPI state. Its expiry
        // returns every context of the current bearer owner to STANDBY and causes
        // exactly one SN-END OF DATA to be emitted by the entity.
        if let Some(issi) = self.bearer_owner
            && self.bearer_ready_deadline.is_some_and(|deadline| now >= deadline)
        {
            for (key, context) in &mut self.contexts {
                if key.issi == issi && matches!(context.state, PdpState::Ready | PdpState::Quiescent) {
                    context.enter_standby(standby_timer_code, now);
                }
            }
            self.bearer_owner = None;
            self.bearer_nsapis.clear();
            self.bearer_ready_deadline = None;
            events.push(TimerEvent::ReadyExpired(issi));
        } else {
            // CONTEXT_READY is per PDP context. Its expiry merely marks that
            // context quiescent; it does not release a bearer still used by other
            // contexts. A fresh TRANSMIT REQUEST reactivates it.
            let keys = self.contexts.keys().copied().collect::<Vec<_>>();
            for key in keys {
                let Some(context) = self.contexts.get_mut(&key) else {
                    continue;
                };
                if context.state == PdpState::Ready
                    && context.ready_deadline.is_some_and(|deadline| now >= deadline)
                {
                    context.state = PdpState::Quiescent;
                    context.ready_deadline = None;
                    events.push(TimerEvent::ContextReadyExpired(key));
                }
            }
        }

        for (key, context) in &self.contexts {
            if matches!(context.state, PdpState::Standby | PdpState::Suspended)
                && context.standby_deadline.is_some_and(|deadline| now >= deadline)
            {
                standby_expired_issis.insert(key.issi);
            }
        }

        // EN 300 392-2 models STANDBY expiry per MS/SNDCP entity: expiry tears
        // down all PDP contexts for that subscriber, including secondary NSAPIs.
        for issi in standby_expired_issis {
            for (key, _) in self.remove_all_for_issi(issi) {
                events.push(TimerEvent::StandbyExpired(key));
            }
        }
        self.clear_bearer_if_empty();
        events
    }

    fn cleanup_subscriber_if_unused(&mut self, issi: u32) {
        if self.contexts.keys().any(|key| key.issi == issi) {
            return;
        }
        self.default_priorities.remove(&issi);
        self.network_endpoint_ids.remove(&issi);
        if self.bearer_owner == Some(issi) {
            self.bearer_owner = None;
            self.bearer_nsapis.clear();
            self.bearer_ready_deadline = None;
        }
    }
}

fn deadline(now: Instant, duration: Option<Duration>) -> Option<Instant> {
    duration.and_then(|duration| now.checked_add(duration))
}

/// CONTEXT_READY code 0 tracks the global READY timer; codes 1..14 use
/// the same duration table, and code 15 is reserved.
pub fn context_ready_timer_duration(context_code: u8, ready_code: u8) -> Option<Duration> {
    match context_code & 0x0f {
        0 => ready_timer_duration(ready_code),
        1..=14 => ready_timer_duration(context_code),
        15 => None,
        _ => unreachable!(),
    }
}

/// EN 300 392-2 READY timer coding. Codes 0 and 15 are reserved.
pub fn ready_timer_duration(code: u8) -> Option<Duration> {
    match code & 0x0f {
        0 => None,
        1 => Some(Duration::from_millis(200)),
        2 => Some(Duration::from_millis(500)),
        3 => Some(Duration::from_millis(700)),
        4 => Some(Duration::from_secs(1)),
        5 => Some(Duration::from_secs(2)),
        6 => Some(Duration::from_secs(3)),
        7 => Some(Duration::from_secs(5)),
        8 => Some(Duration::from_secs(10)),
        9 => Some(Duration::from_secs(20)),
        10 => Some(Duration::from_secs(30)),
        11 => Some(Duration::from_secs(60)),
        12 => Some(Duration::from_secs(120)),
        13 => Some(Duration::from_secs(180)),
        14 => Some(Duration::from_secs(300)),
        15 => None,
        _ => unreachable!(),
    }
}

/// EN 300 392-2 STANDBY timer coding. Code 0 disables the timer; code 15 means
/// that the context remains until explicit deactivation/deregistration.
pub fn standby_timer_duration(code: u8) -> Option<Duration> {
    match code & 0x0f {
        0 => None,
        1 => Some(Duration::from_secs(10)),
        2 => Some(Duration::from_secs(30)),
        3 => Some(Duration::from_secs(60)),
        4 => Some(Duration::from_secs(300)),
        5 => Some(Duration::from_secs(600)),
        6 => Some(Duration::from_secs(1_800)),
        7 => Some(Duration::from_secs(3_600)),
        8 => Some(Duration::from_secs(7_200)),
        9 => Some(Duration::from_secs(10_800)),
        10 => Some(Duration::from_secs(21_600)),
        11 => Some(Duration::from_secs(43_200)),
        12 => Some(Duration::from_secs(86_400)),
        13 => Some(Duration::from_secs(172_800)),
        14 => Some(Duration::from_secs(259_200)),
        15 => None,
        _ => unreachable!(),
    }
}

pub fn response_wait_timer_duration(code: u8) -> Option<Duration> {
    match code & 0x0f {
        0 => Some(Duration::from_millis(400)),
        1 => Some(Duration::from_millis(600)),
        2 => Some(Duration::from_millis(800)),
        3 => Some(Duration::from_secs(1)),
        4 => Some(Duration::from_secs(2)),
        5 => Some(Duration::from_secs(3)),
        6 => Some(Duration::from_secs(4)),
        7 => Some(Duration::from_secs(5)),
        8 => Some(Duration::from_secs(10)),
        9 => Some(Duration::from_secs(15)),
        10 => Some(Duration::from_secs(20)),
        11 => Some(Duration::from_secs(30)),
        12 => Some(Duration::from_secs(40)),
        13 => Some(Duration::from_secs(50)),
        14 => Some(Duration::from_secs(60)),
        15 => None,
        _ => unreachable!(),
    }
}

pub fn mtu_octets(code: u8) -> Option<usize> {
    match code & 7 {
        0 => None,
        1 => Some(296),
        2 => Some(576),
        3 => Some(1_006),
        4 => Some(1_500),
        5 => Some(2_002),
        6 | 7 => None,
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timer_reference_codes_match_profile() {
        assert_eq!(ready_timer_duration(8), Some(Duration::from_secs(10)));
        assert_eq!(standby_timer_duration(4), Some(Duration::from_secs(300)));
        assert_eq!(standby_timer_duration(15), None);
        assert_eq!(response_wait_timer_duration(0), Some(Duration::from_millis(400)));
        assert_eq!(response_wait_timer_duration(7), Some(Duration::from_secs(5)));
        assert_eq!(mtu_octets(2), Some(576));
    }

    #[test]
    fn one_subscriber_can_share_bearer_across_nsapis() {
        let mut table = ContextTable::default();
        assert!(table.claim_bearer(1001, &[2]));
        assert!(table.claim_bearer(1001, &[3, 4]));
        assert!(!table.claim_bearer(1002, &[2]));
        assert_eq!(table.bearer_owner(), Some(1001));
        assert!(!table.release_bearer_nsapis(1001, &[2]));
        assert!(table.release_bearer_nsapis(1001, &[3, 4]));
        assert_eq!(table.bearer_owner(), None);
    }

    #[test]
    fn removing_non_owner_context_does_not_release_owner_nsapi() {
        let now = Instant::now();
        let mut table = ContextTable::default();
        table.insert(ContextKey { issi: 1, nsapi: 2 }, PdpContext::new([10, 0, 0, 2], 4, 576, 4, now));
        table.insert(ContextKey { issi: 2, nsapi: 2 }, PdpContext::new([10, 0, 0, 3], 4, 576, 4, now));
        assert!(table.claim_bearer(1, &[2]));
        table.remove(ContextKey { issi: 2, nsapi: 2 });
        assert_eq!(table.bearer_owner(), Some(1));
        assert_eq!(table.bearer_nsapis().collect::<Vec<_>>(), vec![2]);
    }

    #[test]
    fn snei_is_stable_per_issi_and_unique_between_active_subscribers() {
        let mut table = ContextTable::default();
        let a = table.ensure_network_endpoint_id(0x1234_5678);
        let b = table.ensure_network_endpoint_id(0x2234_5678);
        assert_ne!(a, 0);
        assert_ne!(b, 0);
        assert_ne!(a, b);
        assert_eq!(table.ensure_network_endpoint_id(0x1234_5678), a);
    }

    #[test]
    fn primary_family_contains_secondary_contexts() {
        let now = Instant::now();
        let mut table = ContextTable::default();
        table.insert(ContextKey { issi: 7, nsapi: 2 }, PdpContext::new([10, 0, 0, 2], 4, 576, 4, now));
        let mut secondary = PdpContext::new([10, 0, 0, 2], 4, 576, 4, now);
        secondary.primary_nsapi = Some(2);
        table.insert(ContextKey { issi: 7, nsapi: 3 }, secondary);
        assert_eq!(table.family_nsapis(7, 2), vec![2, 3]);
        assert_eq!(table.family_nsapis(7, 3), vec![3]);
    }

    #[test]
    fn global_ready_expiry_returns_all_contexts_to_standby() {
        let now = Instant::now();
        let first = ContextKey { issi: 42, nsapi: 2 };
        let second = ContextKey { issi: 42, nsapi: 3 };
        let mut table = ContextTable::default();
        for key in [first, second] {
            let mut context = PdpContext::new([10, 0, 0, 2], 4, 576, 4, now);
            context.enter_ready(8, now);
            table.insert(key, context);
        }
        table.claim_bearer(42, &[2, 3]);
        table.refresh_bearer_ready(42, 1, now);
        let events = table.tick(now + Duration::from_millis(201), 4);
        assert_eq!(events, vec![TimerEvent::ReadyExpired(42)]);
        assert_eq!(table.get(first).map(|ctx| ctx.state), Some(PdpState::Standby));
        assert_eq!(table.get(second).map(|ctx| ctx.state), Some(PdpState::Standby));
        assert_eq!(table.bearer_owner(), None);
    }

    #[test]
    fn context_ready_expiry_does_not_release_global_bearer() {
        let now = Instant::now();
        let key = ContextKey { issi: 42, nsapi: 2 };
        let mut table = ContextTable::default();
        let mut context = PdpContext::new([10, 0, 0, 2], 4, 576, 4, now);
        context.enter_ready(1, now);
        table.insert(key, context);
        table.claim_bearer(42, &[2]);
        table.refresh_bearer_ready(42, 8, now);
        let events = table.tick(now + Duration::from_millis(201), 4);
        assert_eq!(events, vec![TimerEvent::ContextReadyExpired(key)]);
        assert_eq!(table.get(key).map(|ctx| ctx.state), Some(PdpState::Quiescent));
        assert_eq!(table.bearer_owner(), Some(42));
    }
    #[test]
    fn primary_reactivation_may_reuse_address_shared_with_secondary() {
        let now = Instant::now();
        let mut table = ContextTable::default();
        let primary_key = ContextKey { issi: 7, nsapi: 2 };
        table.insert(primary_key, PdpContext::new([10, 0, 0, 2], 4, 576, 4, now));
        let mut secondary = PdpContext::new([10, 0, 0, 2], 4, 576, 4, now);
        secondary.primary_nsapi = Some(2);
        table.insert(ContextKey { issi: 7, nsapi: 3 }, secondary);
        assert!(!table.address_in_use_by_other(primary_key, [10, 0, 0, 2]));
    }

    #[test]
    fn standby_expiry_removes_all_contexts_and_subscriber_identity() {
        let now = Instant::now();
        let mut table = ContextTable::default();
        let primary_key = ContextKey { issi: 9, nsapi: 2 };
        let secondary_key = ContextKey { issi: 9, nsapi: 3 };
        table.insert(primary_key, PdpContext::new([10, 0, 0, 9], 4, 576, 1, now));
        let mut secondary = PdpContext::new([10, 0, 0, 9], 4, 576, 14, now);
        secondary.primary_nsapi = Some(2);
        table.insert(secondary_key, secondary);
        let snei = table.ensure_network_endpoint_id(9);
        assert_ne!(snei, 0);

        let events = table.tick(now + Duration::from_secs(11), 1);
        assert!(events.contains(&TimerEvent::StandbyExpired(primary_key)));
        assert!(events.contains(&TimerEvent::StandbyExpired(secondary_key)));
        assert_eq!(table.contexts_for_issi(9), 0);
        assert_eq!(table.network_endpoint_id(9), None);
    }

}
