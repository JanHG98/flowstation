#!/usr/bin/env python3
"""Static Package-C guard for the TLMC runtime.

This checker intentionally uses only the Python standard library so it can run
before Cargo in CI and on a freshly installed TBS.  It does not replace Rust
compilation; it protects the architecture and routing invariants that are easy
to regress while Package C is still evolving.
"""

from __future__ import annotations

from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    target = ROOT / path
    if not target.is_file():
        fail(f"missing required file: {path}")
    return target.read_text(encoding="utf-8")


def fail(message: str) -> None:
    print(f"TLMC Package C check failed: {message}", file=sys.stderr)
    raise SystemExit(1)


def require(text: str, markers: list[str], label: str) -> None:
    missing = [marker for marker in markers if marker not in text]
    if missing:
        fail(f"{label} is missing: {', '.join(missing)}")


def extract_function(text: str, name: str) -> str:
    marker = re.search(rf"\bfn\s+{re.escape(name)}\b", text)
    if marker is None:
        fail(f"function {name} not found")
    brace = text.find("{", marker.end())
    if brace < 0:
        fail(f"function {name} has no body")
    depth = 0
    for index in range(brace, len(text)):
        char = text[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return text[marker.start() : index + 1]
    fail(f"function {name} has an unbalanced body")
    return ""


def strip_comments_and_strings(text: str) -> str:
    # Good enough for a delimiter sanity check. Raw strings are replaced first.
    text = re.sub(r'r#+".*?"#+', '""', text, flags=re.S)
    text = re.sub(r'r".*?"', '""', text, flags=re.S)
    text = re.sub(r'//.*', '', text)
    text = re.sub(r'/\*.*?\*/', '', text, flags=re.S)
    text = re.sub(r'"(?:\\.|[^"\\])*"', '""', text, flags=re.S)
    text = re.sub(r"'(?:\\.|[^'\\])'", "''", text, flags=re.S)
    return text


def check_delimiters(path: str, text: str) -> None:
    # Rust format strings and lifetimes make a dependency-free lexical stripper
    # surprisingly error-prone. The architecture guard therefore uses a
    # conservative raw-count check; Cargo/rustc in the following CI step remains
    # the authoritative parser.
    for opening, closing in (("{", "}"), ("(", ")"), ("[", "]")):
        if text.count(opening) != text.count(closing):
            fail(
                f"unbalanced {opening}{closing} delimiters in {path}: "
                f"{text.count(opening)} opening, {text.count(closing)} closing"
            )


def main() -> None:
    runtime_path = "crates/tetra-entities/src/umac/tlmc_runtime.rs"
    ms_path = "crates/tetra-entities/src/umac/umac_ms.rs"
    bs_path = "crates/tetra-entities/src/umac/umac_bs.rs"
    mle_path = "crates/tetra-entities/src/mle/mle_bs.rs"
    mle_ms_path = "crates/tetra-entities/src/mle/mle_ms.rs"
    tlmc_path = "crates/tetra-saps/src/tlmc/mod.rs"
    sapmsg_path = "crates/tetra-saps/src/sapmsg.rs"
    test_path = "crates/tetra-entities/tests/test_tlmc_runtime.rs"

    runtime = read(runtime_path)
    ms = read(ms_path)
    bs = read(bs_path)
    mle = read(mle_path)
    mle_ms = read(mle_ms_path)
    tlmc = read(tlmc_path)
    sapmsg = read(sapmsg_path)
    tests = read(test_path)
    module = read("crates/tetra-entities/src/umac/mod.rs")

    require(module, ["pub mod tlmc_runtime;"], "UMAC module")
    require(
        runtime,
        [
            "pub struct TlmcRuntime",
            "pub struct TlmcRuntimeSnapshot",
            "pub enum TlmcRuntimeError",
            "pub fn apply_configure",
            "pub fn resource_transition",
            "pub fn record_measurement",
            "pub fn set_monitor_list",
            "pub fn record_monitor",
            "pub fn set_assessment_list",
            "pub fn record_assessment",
            "pub fn begin_scan",
            "pub fn complete_scan",
            "pub fn begin_cell_read",
            "pub fn complete_cell_read",
            "pub fn begin_select",
            "pub fn complete_select",
            "pub fn receive_select_indication",
            "pub fn respond_select",
        ],
        "TLMC runtime",
    )
    if "unimplemented!(" in runtime or "todo!(" in runtime:
        fail("TLMC runtime contains an active unimplemented!/todo! macro")

    configure = extract_function(runtime, "apply_configure")
    require(configure, ["validate_configure", "merge_configure", "configure_confirmation"], "configure runtime")

    require(
        tlmc,
        [
            "pub struct TlmcConfigureInd",
            "pub lower_layer_resource_availability: LowerLayerResourceAvailability",
            "pub reason: LowerLayerResourceReason",
        ],
        "TLMC resource indication",
    )

    require(
        ms,
        [
            "tlmc: TlmcRuntime",
            "pub fn tlmc_snapshot",
            "fn rx_tlmc_prim",
            "fn observe_tlmc_channel",
            "fn observe_tlmc_sysinfo",
            "fn expire_tlmc_operations",
            "TlmcConfigureReq",
            "TlmcMonitorListReq",
            "TlmcAssessmentListReq",
            "TlmcScanReq",
            "TlmcCellReadReq",
            "TlmcSelectReq",
            "TlmcSelectResp",
            "LowerLayerResourceAvailability::Unavailable",
            "LowerLayerResourceAvailability::Available",
            "Layer2Report::ServiceTemporarilyUnavailable",
            "Sap::TlmcSap =>",
        ],
        "UMAC-MS TLMC adapter",
    )
    rx_ms = extract_function(ms, "rx_tlmc_prim")
    if "unimplemented!(" in rx_ms or "unimplemented_log!(" in rx_ms:
        fail("UMAC-MS TLMC dispatcher still contains an unimplemented path")

    require(
        bs,
        [
            "tlmc: TlmcRuntime",
            "pub fn tlmc_snapshot",
            "fn rx_tlmc_prim",
            "Layer2Report::ServiceNotSupported",
            "Sap::TlmcSap =>",
        ],
        "UMAC-BS defensive TLMC adapter",
    )
    rx_bs = extract_function(bs, "rx_tlmc_prim")
    if "unimplemented!(" in rx_bs or "unimplemented_log!(" in rx_bs:
        fail("UMAC-BS TLMC dispatcher still contains an unimplemented path")

    require(
        mle,
        [
            "fn rx_tlmc_prim",
            "TlmcConfigureInd",
            "TlmcMeasurementInd",
            "TlmcMonitorInd",
            "TlmcAssessmentInd",
            "TlmcScanConf",
            "TlmcCellReadConf",
            "TlmcSelectConf",
            "TlmcReportInd",
        ],
        "MLE TLMC consumer",
    )
    rx_mle = extract_function(mle, "rx_tlmc_prim")
    if "unimplemented!(" in rx_mle or "unimplemented_log!(" in rx_mle:
        fail("MLE TLMC consumer still contains an unimplemented path")

    require(mle_ms, ["fn rx_tlmc_prim", "TlmcConfigureInd", "TlmcSelectConf"], "MLE-MS TLMC consumer")
    rx_mle_ms = extract_function(mle_ms, "rx_tlmc_prim")
    if "unimplemented!(" in rx_mle_ms or "unimplemented_log!(" in rx_mle_ms:
        fail("MLE-MS TLMC consumer still contains an unimplemented path")

    require(
        sapmsg,
        [
            "TlmcConfigureInd(TlmcConfigureInd)",
            "TlmcMeasurementInd(TlmcMeasurementInd)",
            "TlmcMonitorInd(TlmcMonitorInd)",
            "TlmcScanConf(TlmcScanConf)",
            "TlmcSelectConf(TlmcSelectConf)",
        ],
        "SapMsgInner TLMC variants",
    )

    require(
        tests,
        [
            "configure_request_returns_confirmation",
            "scan_completes_on_valid_sync_for_requested_carrier",
            "resource_loss_and_recovery_are_edge_triggered",
            "runtime_snapshot_is_available_for_tbs_diagnostics",
            "scan_times_out_with_negative_confirmation",
        ],
        "TLMC integration tests",
    )
    if "matches!(message.msg" in tests:
        fail("integration tests move SapMsgInner out of a borrowed message")

    for path, text in [
        (runtime_path, runtime),
        (ms_path, ms),
        (bs_path, bs),
        (mle_path, mle),
        (mle_ms_path, mle_ms),
        (tlmc_path, tlmc),
        (test_path, tests),
    ]:
        check_delimiters(path, text)

    print("SWMI Foundation 1 Package C static checks passed.")
    print("  TLMC runtime state machine: present")
    print("  UMAC-MS runtime adapter: present")
    print("  UMAC-BS defensive routing: present")
    print("  resource loss/recovery and operation timeout paths: present")
    print("  TBS diagnostics snapshot: present")


if __name__ == "__main__":
    main()
