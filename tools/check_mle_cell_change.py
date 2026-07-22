#!/usr/bin/env python3
"""Dependency-free architecture guard for SWMI Mobility 1 Package A."""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def fail(message: str) -> None:
    print(f"MLE cell-change check failed: {message}", file=sys.stderr)
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
    common = read("crates/tetra-saps/src/common/mod.rs")
    control = read("crates/tetra-saps/src/control/mle_cell_change.rs")
    sapmsg = read("crates/tetra-saps/src/sapmsg.rs")
    lcmc = read("crates/tetra-saps/src/lcmc/mod.rs")
    runtime = read("crates/tetra-entities/src/mle/cell_change_runtime.rs")
    mle_bs = read("crates/tetra-entities/src/mle/mle_bs.rs")
    cmce_bs = read("crates/tetra-entities/src/cmce/cmce_bs.rs")
    pdu_tests = read("crates/tetra-pdus/tests/test_mle_cell_change_pdus.rs")
    runtime_tests = read("crates/tetra-entities/tests/test_mle_cell_change_runtime.rs")
    harness = read("crates/tetra-entities/tests/common/two_cell.rs")
    two_cell = read("crates/tetra-entities/tests/test_two_cell_mobility.rs")

    require(
        common,
        [
            "pub enum MleChannelCommandValid",
            "pub enum MleFailCause",
            "pub enum MleChannelResponseType",
            "pub enum MleChannelRequestReason",
            "pub enum MleChannelRequestRetryDelay",
        ],
        "typed MLE values",
    )
    require(
        control,
        [
            "GrantPrepare",
            "RejectPrepare",
            "AcknowledgeRestore",
            "RejectRestore",
            "RespondChannelRequest",
        ],
        "local MLE control interface",
    )
    require(
        sapmsg,
        ["MleCellChangeControl(MleCellChangeControl)", "LcmcMleRestoreInd(LcmcMleRestoreInd)"],
        "SAP message wiring",
    )
    require(
        lcmc,
        [
            "pub struct LcmcMleRestoreInd",
            "pub previous_mcc: Option<u16>",
            "pub previous_location_area: Option<u16>",
        ],
        "CMCE restore indication",
    )
    require(
        runtime,
        [
            "pub struct MleCellChangeRuntime",
            "pub enum MleCellChangePhase",
            "PrepareReceived",
            "RestoreReceived",
            "ChannelRequestReceived",
            "pub fn observe_prepare",
            "pub fn observe_restore",
            "pub fn observe_channel_request",
            "pub fn handle_control",
            "pub fn tick",
            "pub fn snapshot",
            "CELL_CHANGE_TRANSACTION_TIMEOUT_SLOTS",
            "DNewCell",
            "DPrepareFail",
            "DRestoreAck",
            "DRestoreFail",
            "DChannelResponse",
            "parse_errors",
            "invalid_controls",
            "timeouts",
        ],
        "cell-change transaction runtime",
    )
    require(
        mle_bs,
        [
            "cell_change: MleCellChangeRuntime",
            "pub fn cell_change_snapshot",
            "MlePduTypeUl::UPrepare",
            "MlePduTypeUl::URestore",
            "MlePduTypeUl::UChannelRequest",
            "LcmcMleRestoreInd",
            "fn queue_cell_change_outbound",
            "SapMsgInner::MleCellChangeControl",
            "self.cell_change.tick(ts)",
        ],
        "MLE-BS runtime integration",
    )
    require(
        cmce_bs,
        [
            "pub fn rx_lcmc_mle_restore_ind",
            "SapMsgInner::LcmcMleRestoreInd",
            "MleCellChangeControl::RejectRestore",
            "RestorationCannotBeDoneOnCell",
        ],
        "CMCE conservative restore fallback",
    )

    pdu_files = [
        "crates/tetra-pdus/src/mle/pdus/d_new_cell.rs",
        "crates/tetra-pdus/src/mle/pdus/d_prepare_fail.rs",
        "crates/tetra-pdus/src/mle/pdus/d_restore_ack.rs",
        "crates/tetra-pdus/src/mle/pdus/d_restore_fail.rs",
        "crates/tetra-pdus/src/mle/pdus/d_channel_response.rs",
        "crates/tetra-pdus/src/mle/pdus/u_prepare.rs",
        "crates/tetra-pdus/src/mle/pdus/u_restore.rs",
        "crates/tetra-pdus/src/mle/pdus/u_channel_request.rs",
    ]
    for relative in pdu_files:
        text = read(relative)
        if "unimplemented!" in text or "todo!" in text:
            fail(f"runtime placeholder remains in {relative}")
        require(text, ["from_bitbuf", "to_bitbuf"], relative)
        balanced(relative, text)

    require(
        pdu_tests,
        [
            "downlink_baseline_vectors_match_the_normative_field_order",
            "cell_change_pdus_roundtrip_and_preserve_long_embedded_sdus",
            "channel_request_and_response_roundtrip",
        ],
        "PDU codec tests",
    )
    require(
        runtime_tests,
        [
            "prepare_can_be_granted_deferred_and_rejected",
            "restore_acknowledgement_and_failure_use_the_learned_local_route",
            "channel_request_response_and_invalid_transition_are_accounted",
            "pending_transactions_receive_deterministic_timeout_responses",
        ],
        "runtime tests",
    )
    require(
        harness,
        [
            "pub fn submit_u_prepare",
            "pub fn submit_u_restore",
            "pub fn submit_u_channel_request",
            "pub fn control_cell_change",
            "pub fn cell_change_snapshot",
        ],
        "two-cell harness extension",
    )
    require(
        two_cell,
        [
            "prepare_on_old_cell_and_restore_on_target_cell_are_isolated",
            "DNewCell::from_bitbuf",
            "DRestoreAck::from_bitbuf",
            "LcmcMleRestoreInd",
        ],
        "two-cell mobility test",
    )

    for relative, text in (
        ("crates/tetra-saps/src/common/mod.rs", common),
        ("crates/tetra-saps/src/control/mle_cell_change.rs", control),
        ("crates/tetra-saps/src/sapmsg.rs", sapmsg),
        ("crates/tetra-saps/src/lcmc/mod.rs", lcmc),
        ("crates/tetra-entities/src/mle/cell_change_runtime.rs", runtime),
        ("crates/tetra-entities/src/mle/mle_bs.rs", mle_bs),
        ("crates/tetra-entities/src/cmce/cmce_bs.rs", cmce_bs),
        ("crates/tetra-pdus/tests/test_mle_cell_change_pdus.rs", pdu_tests),
        ("crates/tetra-entities/tests/test_mle_cell_change_runtime.rs", runtime_tests),
        ("crates/tetra-entities/tests/common/two_cell.rs", harness),
        ("crates/tetra-entities/tests/test_two_cell_mobility.rs", two_cell),
    ):
        balanced(relative, text)

    print("SWMI Mobility 1 Package A static checks passed.")
    print("  typed uplink/downlink MLE cell-change PDUs: present")
    print("  infrastructure transaction registry and timeouts: present")
    print("  MM/CMCE indications and local control responses: present")
    print("  conservative CMCE restore rejection until call-restore phase: present")
    print("  TBS WebUI-ready diagnostic snapshot: present")
    print("  two-cell prepare/restore acceptance path: present")


if __name__ == "__main__":
    main()
