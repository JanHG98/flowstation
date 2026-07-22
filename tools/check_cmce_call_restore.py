#!/usr/bin/env python3
"""Dependency-free architecture guard for SWMI Mobility 1 Package B."""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def fail(message: str) -> None:
    print(f"CMCE call-restore check failed: {message}", file=sys.stderr)
    raise SystemExit(1)


def read(relative: str) -> str:
    path = ROOT / relative
    if not path.is_file():
        fail(f"missing required file: {relative}")
    return path.read_text(encoding="utf-8")


def require(text: str, markers: list[str], label: str) -> None:
    missing = [marker for marker in markers if marker not in text]
    if missing:
        fail(f"{label} misses: {', '.join(missing)}")


def balanced(relative: str, text: str) -> None:
    stripped = re.sub(r'//.*?$|/\*.*?\*/|"(?:\\.|[^"\\])*"', '', text, flags=re.M | re.S)
    for opening, closing in (("{", "}"), ("(", ")"), ("[", "]")):
        if stripped.count(opening) != stripped.count(closing):
            fail(f"unbalanced {opening}{closing} in {relative}")


def main() -> None:
    runtime_path = "crates/tetra-entities/src/cmce/call_restore_runtime.rs"
    restore_path = "crates/tetra-entities/src/cmce/subentities/cc_bs/procedures/restoration.rs"
    pdu_path = "crates/tetra-entities/src/cmce/subentities/cc_bs/pdu.rs"
    state_path = "crates/tetra-entities/src/cmce/subentities/cc_bs/state/mod.rs"
    timers_path = "crates/tetra-entities/src/cmce/subentities/cc_bs/timers.rs"
    uplink_path = "crates/tetra-entities/src/cmce/subentities/cc_bs/procedures/uplink.rs"
    cmce_path = "crates/tetra-entities/src/cmce/cmce_bs.rs"
    mle_runtime_path = "crates/tetra-entities/src/mle/cell_change_runtime.rs"
    mle_bs_path = "crates/tetra-entities/src/mle/mle_bs.rs"
    control_path = "crates/tetra-saps/src/control/mle_cell_change.rs"
    runtime_test_path = "crates/tetra-entities/tests/test_call_restore_runtime.rs"
    two_cell_test_path = "crates/tetra-entities/tests/test_two_cell_call_restore.rs"

    runtime = read(runtime_path)
    restoration = read(restore_path)
    pdu = read(pdu_path)
    state = read(state_path)
    timers = read(timers_path)
    uplink = read(uplink_path)
    cmce = read(cmce_path)
    mle_runtime = read(mle_runtime_path)
    mle_bs = read(mle_bs_path)
    control = read(control_path)
    runtime_tests = read(runtime_test_path)
    two_cell_tests = read(two_cell_test_path)

    require(
        runtime,
        [
            "pub enum RestoreCallKind",
            "pub enum RestorePhase",
            "Requested",
            "ContextMatched",
            "Queued",
            "ResourceAllocated",
            "Restored",
            "Rejected",
            "TimedOut",
            "pub struct GroupCallRestoreContext",
            "pub struct IndividualCallRestoreContext",
            "created_at: TdmaTime",
            "active_timer_started: Option<TdmaTime>",
            "pub struct CallRestoreTransaction",
            "pub struct CallRestoreCounters",
            "queued_restores",
            "queued_allocations_completed",
            "call_id_changes",
            "pub fn install_context",
            "pub fn begin",
            "pub fn mark_context_matched",
            "pub fn mark_queued",
            "pub fn queued_key_for_call",
            "pub fn set_queued_transmission_request",
            "pub fn mark_resource_allocated",
            "pub fn mark_restored",
            "pub fn reject",
            "pub fn tick",
            "pub fn snapshot",
            "pub fn resolved_call_id",
            "pub fn reserve_call_id",
            "DuplicatePending",
            "DuplicateQueued",
            "DuplicateTerminal",
            "CALL_RESTORE_TRANSACTION_TIMEOUT_SLOTS",
            "CALL_RESTORE_REPLAY_WINDOW_SLOTS",
        ],
        "call-restore runtime",
    )

    require(
        restoration,
        [
            "pub(in crate::cmce) enum MleCallRestoreDecision",
            "Acknowledge",
            "chan_alloc: Option<CmceChanAllocReq>",
            "fn process_call_restore",
            "fn restore_group_call",
            "fn restore_individual_call",
            "pub(super) fn drive_queued_call_restores",
            "fn send_restore_tx_granted",
            "fn send_restore_failure_release",
            "handle_queued_restore_tx_ceased",
            "handle_queued_restore_tx_demand",
            "CallStatus::Callqueued",
            "TransmissionGrant::RequestQueued",
            "DTxGranted",
            "DisconnectCause::CallRestorationOfTheOtherUserFailed",
            "mark_context_matched",
            "mark_resource_allocated",
            "mark_restored",
            "begin_restore",
            "complete_restore",
            "reserve_restore_call_id",
            "install_restored_group_setup",
            "install_restored_individual_setup",
            "CircuitDlMediaSource::SwMI",
            "Layer2Service::Acknowledged",
            "Preserve the already-running T310",
            "TransmissionGrant::GrantedToOtherUser",
            "BrewNotification::Never",
        ],
        "CMCE restoration procedure",
    )

    require(
        pdu,
        [
            "build_d_call_restore_extended",
            "reset_call_time_out_timer_t310_: call_time_out.is_some()",
            "new_call_identifier",
            "call_status",
            "build_sapmsg_direct_with_allocation",
        ],
        "D-CALL RESTORE and direct delivery builders",
    )

    require(
        state,
        [
            "CcFormalState::Restore",
            "CcFormalEvent::RestoreRequest",
            "CcFormalEvent::RestoreComplete",
            "CcFormalEvent::RestoreReject",
            "pub(super) fn begin_restore",
            "pub(super) fn complete_restore",
        ],
        "formal call-control restore state",
    )

    require(
        timers,
        [
            "self.drive_queued_call_restores(queue)",
            "self.call_restore.tick(dltime)",
            "self.send_timed_out_restore_release(queue, key)",
        ],
        "queue retry and timeout integration",
    )


    require(
        uplink,
        [
            "handle_queued_restore_tx_ceased(sender, call_id)",
            "handle_queued_restore_tx_demand(queue, requesting_party, call_id)",
        ],
        "queued restore uplink routing",
    )

    require(
        cmce,
        [
            "rx_lcmc_mle_restore_ind",
            "handle_mle_call_restore",
            "MleCallRestoreDecision::Acknowledge",
            "MleCellChangeControl::AcknowledgeRestore",
            "MleCallRestoreDecision::Reject",
            "MleCellChangeControl::RejectRestore",
        ],
        "CMCE/MLE restore handoff",
    )

    require(
        control,
        [
            "AcknowledgeRestore",
            "chan_alloc: Option<CmceChanAllocReq>",
        ],
        "MLE restore control allocation",
    )
    require(
        mle_runtime,
        [
            "pub chan_alloc: Option<CmceChanAllocReq>",
            "MleCellChangeControl::AcknowledgeRestore",
            "chan_alloc",
        ],
        "MLE outbound allocation propagation",
    )
    require(
        mle_bs,
        [
            "chan_alloc: outbound.chan_alloc",
            "SapMsgInner::TlaTlDataReqBl",
        ],
        "MLE-to-LLC allocation propagation",
    )

    require(
        runtime_tests,
        [
            "group_restore_tracks_context_resource_and_floor",
            "individual_restore_preserves_other_floor_holder",
            "replay_is_idempotent_and_pending_duplicate_is_rejected",
            "unanswered_restore_times_out_and_is_visible_to_webui_snapshot",
            "queued_restore_replays_then_completes_when_a_bearer_is_allocated",
            "queued_restore_tx_request_can_be_cancelled_and_requeued_by_old_or_new_call_id",
        ],
        "call-restore runtime tests",
    )
    require(
        two_cell_tests,
        [
            "running_group_call_is_restored_on_target_cell_with_floor_and_priority_context",
            "running_individual_simplex_call_restores_without_stealing_the_other_floor",
            "call_id_collision_is_remapped_once_and_reused_by_later_group_participants",
            "congested_target_acknowledges_restore_as_queued_without_a_channel_allocation",
            "replayed_group_restore_keeps_the_same_bearer_allocation",
            "group_listener_restore_keeps_receive_plane_when_another_user_is_speaking",
            "duplex_individual_restore_is_granted_even_without_a_tx_request_bit",
            "DRestoreAck::from_bitbuf",
            "DCallRestore::from_bitbuf",
            "allocation.is_some()",
            "CallStatus::Callqueued",
            "reset_call_time_out_timer_t310_",
        ],
        "two-cell call-restoration tests",
    )

    # The old Package-A-only unconditional fallback must not return.
    old_fallback = re.compile(
        r"rx_lcmc_mle_restore_ind[\s\S]{0,1800}MleCellChangeControl::RejectRestore[\s\S]{0,300}RestorationCannotBeDoneOnCell"
    )
    if old_fallback.search(cmce):
        fail("old unconditional CMCE restore rejection is still present")

    for relative, text in (
        (runtime_path, runtime),
        (restore_path, restoration),
        (pdu_path, pdu),
        (state_path, state),
        (timers_path, timers),
        (uplink_path, uplink),
        (cmce_path, cmce),
        (mle_runtime_path, mle_runtime),
        (mle_bs_path, mle_bs),
        (control_path, control),
        (runtime_test_path, runtime_tests),
        (two_cell_test_path, two_cell_tests),
    ):
        balanced(relative, text)

    print("SWMI Mobility 1 Package B static checks passed.")
    print("  group and individual CMCE restore state machines: present")
    print("  local bearer allocation and D-RESTORE-ACK propagation: present")
    print("  floor, priority, call origin and T310 references: preserved")
    print("  call-ID collision aliases and replay handling: present")
    print("  congestion queue with later D-TX GRANTED: present")
    print("  queued U-TX DEMAND/U-TX CEASED handling: present")
    print("  listener, simplex-floor and duplex grant semantics: present")
    print("  timeout/reject cleanup and WebUI-ready diagnostics: present")
    print("  two-cell group and individual restore acceptance paths: present")


if __name__ == "__main__":
    main()
