#!/usr/bin/env python3
"""Dependency-free architecture guard for SWMI Foundation 1 Package E."""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def fail(message: str) -> None:
    print(f"Foundation Package E check failed: {message}", file=sys.stderr)
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


def delimiters(relative: str, text: str) -> None:
    for opening, closing in (("{", "}"), ("(", ")"), ("[", "]")):
        if text.count(opening) != text.count(closing):
            fail(f"unbalanced {opening}{closing} in {relative}")


def function_body(text: str, name: str) -> str:
    match = re.search(rf"\bfn\s+{re.escape(name)}\b", text)
    if not match:
        fail(f"function {name} not found")
    start = text.find("{", match.end())
    if start < 0:
        fail(f"function {name} has no body")
    depth = 0
    for index in range(start, len(text)):
        if text[index] == "{":
            depth += 1
        elif text[index] == "}":
            depth -= 1
            if depth == 0:
                return text[start : index + 1]
    fail(f"function {name} has an unclosed body")


def main() -> None:
    runtime_path = "crates/tetra-entities/src/mle/ltpd_runtime.rs"
    router_path = "crates/tetra-entities/src/messagerouter.rs"
    integration_path = "crates/tetra-entities/tests/test_ltpd_runtime.rs"
    harness_path = "crates/tetra-entities/tests/common/two_cell.rs"
    two_cell_test_path = "crates/tetra-entities/tests/test_two_cell_foundation.rs"

    runtime = read(runtime_path)
    router = read(router_path)
    integration = read(integration_path)
    harness = read(harness_path)
    two_cell_tests = read(two_cell_test_path)

    require(
        runtime,
        [
            "TxReporter",
            "TxState",
            "COMPLETED_HANDLE_RETENTION_SLOTS",
            "LtpdPendingTransferSnapshot",
            "LtpdCompletedTransferSnapshot",
            "completed: HashMap<RequestHandle, CompletedTransfer>",
            "duplicate_handle_rejections",
            "cancelled_transfers",
            "timed_out_transfers",
            "invalid_transition_rejections",
            "fn complete_transfer",
            "fn fail_all_pending",
            "RESULT_DUPLICATE_HANDLE",
            "RESULT_CANCEL_TOO_LATE",
            "pending.tx_reporter.get_state()",
            "pending.tx_reporter.is_in_final_state()",
            "tx_reporter: Some(tx_reporter)",
        ],
        "robust LTPD transaction runtime",
    )

    unitdata = function_body(runtime, "unitdata")
    if "self.pending.remove" in unitdata:
        fail("unitdata still completes transfers synchronously")
    if "TransferResult::SuccessBufferEmpty" in unitdata:
        fail("unitdata still reports success before TxReporter completion")
    if "self.completed.contains_key(&request.handle)" not in unitdata:
        fail("completed-handle replay guard missing from unitdata")

    tick = function_body(runtime, "tick")
    for marker in (
        "TxState::Discarded",
        "TxState::Lost",
        "TxState::Acknowledged",
        "TRANSFER_TIMEOUT_SLOTS",
        "self.complete_transfer",
    ):
        if marker not in tick:
            fail(f"tick misses transaction outcome marker: {marker}")

    require(router, ["pub fn iter(&self) -> impl Iterator<Item = &SapMsg>"], "MessageQueue inspection")

    require(
        integration,
        [
            "duplicate_handle_is_rejected_while_original_transfer_is_pending",
            "cancel_removes_pending_transfer_and_reports_failure",
            "pending_transfer_times_out_without_llc_or_mac_progress",
            "reconnect_from_already_connected_state_is_rejected",
            "reporter.mark_transmitted()",
            "reporter.mark_acknowledged()",
        ],
        "Package E LTPD integration tests",
    )

    require(
        harness,
        [
            "pub struct TwoCellHarness",
            "pub enum TestCell",
            "pub fn learn_route",
            "pub fn transfer_route",
            "pub fn set_resources_available",
            "pub fn ltpd_snapshot",
            "main_carrier = 1521",
            "main_carrier = 1522",
        ],
        "two-cell harness",
    )

    require(
        two_cell_tests,
        [
            "two_cells_keep_independent_identity_and_packet_contexts",
            "packet_context_can_move_between_cells_without_cross_contamination",
            "lower_layer_failure_is_isolated_to_one_cell",
        ],
        "two-cell acceptance tests",
    )

    for relative, text in (
        (runtime_path, runtime),
        (router_path, router),
        (integration_path, integration),
        (harness_path, harness),
        (two_cell_test_path, two_cell_tests),
    ):
        delimiters(relative, text)

    print("SWMI Foundation 1 Package E static checks passed.")
    print("  TxReporter-backed completion: present")
    print("  duplicate/replay handle guard: present")
    print("  cancel and bounded timeout handling: present")
    print("  negative lifecycle transitions: present")
    print("  WebUI-ready robustness counters: present")
    print("  reusable two-cell test harness: present")


if __name__ == "__main__":
    main()
