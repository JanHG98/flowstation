#!/usr/bin/env python3
"""Static guardrails for SWMI Foundation 1 Package B.

This checker is deliberately dependency-free. It does not replace rustc, but it
catches the easy-to-miss integration regressions in the typed TLMC/LTPD
foundation before the full Rust build runs.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

COMMON = ROOT / "crates/tetra-saps/src/common/mod.rs"
TLMC = ROOT / "crates/tetra-saps/src/tlmc/mod.rs"
LTPD = ROOT / "crates/tetra-saps/src/ltpd/mod.rs"
SAPMSG = ROOT / "crates/tetra-saps/src/sapmsg.rs"
ADDRESS = ROOT / "crates/tetra-core/src/address.rs"
MLE_BS = ROOT / "crates/tetra-entities/src/mle/mle_bs.rs"
MLE_MS = ROOT / "crates/tetra-entities/src/mle/mle_ms.rs"
TESTS = ROOT / "crates/tetra-saps/tests/swmi_foundation_types.rs"

EXPECTED_COMMON = {
    "ChannelChangeHandle",
    "ChannelChangeDecision",
    "LowerLayerResourceAvailability",
    "LowerLayerResourceReason",
    "CellIdentity",
    "CellCandidate",
    "CellServiceLevel",
    "MeasurementValue",
    "MeasurementReport",
    "ScanRequestId",
    "SelectionCause",
    "SelectionResult",
    "OperatingMode",
    "CallReleaseInstruction",
    "DataPriority",
    "Layer2Qos",
    "ReservationInfo",
    "TransferResult",
    "SetupReport",
    "SndcpStatus",
    "RestoreContext",
    "MleCellState",
    "ChannelChangeState",
    "TlmcScanState",
    "TlmcSelectionState",
    "LtpdLinkState",
    "LtpdContextKey",
}

EXPECTED_TLMC = {
    "TlmcAssessmentInd",
    "TlmcAssessmentListReq",
    "TlmcCellReadReq",
    "TlmcCellReadConf",
    "TlmcConfigureInd",
    "TlmcConfigureReq",
    "TlmcConfigureConf",
    "TlmcMeasurementInd",
    "TlmcMonitorInd",
    "TlmcMonitorListReq",
    "TlmcReportInd",
    "TlmcScanReq",
    "TlmcScanConf",
    "TlmcScanReportInd",
    "TlmcSelectReq",
    "TlmcSelectInd",
    "TlmcSelectResp",
    "TlmcSelectConf",
}

EXPECTED_LTPD = {
    "LtpdMleActivityReq",
    "LtpdMleBreakInd",
    "LtpdMleBusyInd",
    "LtpdMleCancelReq",
    "LtpdMleCloseInd",
    "LtpdMleConfigureReq",
    "LtpdMleConfigureInd",
    "LtpdMleConnectReq",
    "LtpdMleConnectInd",
    "LtpdMleConnectResp",
    "LtpdMleConnectConfirm",
    "LtpdMleDisableInd",
    "LtpdMleDisconnectReq",
    "LtpdMleDisconnectInd",
    "LtpdMleEnableInd",
    "LtpdMleInfoInd",
    "LtpdMleIdleInd",
    "LtpdMleOpenInd",
    "LtpdMleReceiveInd",
    "LtpdMleReconnectReq",
    "LtpdMleReconnectInd",
    "LtpdMleReconnectConfirm",
    "LtpdMleReleaseReq",
    "LtpdMleReportInd",
    "LtpdMleResumeInd",
    "LtpdMleUnitdataReq",
    "LtpdMleUnitdataInd",
}


def read(path: Path) -> str:
    if not path.is_file():
        fail(f"missing required file: {path.relative_to(ROOT)}")
    return path.read_text(encoding="utf-8")


def fail(message: str) -> None:
    print(f"ERROR: {message}", file=sys.stderr)
    raise SystemExit(1)


def defined_types(text: str) -> set[str]:
    return set(re.findall(r"(?m)^pub\s+(?:struct|enum|type)\s+(\w+)", text))


def check_delimiters(path: Path, text: str) -> None:
    pairs = {"}": "{", ")": "(", "]": "["}
    stack: list[tuple[str, int]] = []
    state = "code"
    i = 0
    while i < len(text):
        ch = text[i]
        nxt = text[i + 1] if i + 1 < len(text) else ""
        if state == "code":
            if ch == "/" and nxt == "/":
                state = "line_comment"
                i += 2
                continue
            if ch == "/" and nxt == "*":
                state = "block_comment"
                i += 2
                continue
            if ch == '"':
                state = "string"
                i += 1
                continue
            if ch == "'" and nxt not in {"_"}:
                # Lifetimes are ignored; only enter char mode for obvious quoted chars.
                closing = text.find("'", i + 1, min(len(text), i + 8))
                if closing != -1:
                    state = "char"
                    i += 1
                    continue
            if ch in "{([":
                stack.append((ch, i))
            elif ch in "})]":
                if not stack or stack[-1][0] != pairs[ch]:
                    fail(f"unbalanced delimiter in {path.relative_to(ROOT)} near byte {i}")
                stack.pop()
        elif state == "line_comment":
            if ch == "\n":
                state = "code"
        elif state == "block_comment":
            if ch == "*" and nxt == "/":
                state = "code"
                i += 2
                continue
        elif state == "string":
            if ch == "\\":
                i += 2
                continue
            if ch == '"':
                state = "code"
        elif state == "char":
            if ch == "\\":
                i += 2
                continue
            if ch == "'":
                state = "code"
        i += 1
    if stack:
        fail(f"unclosed delimiter in {path.relative_to(ROOT)}")


def check_expected_types(path: Path, text: str, expected: set[str]) -> None:
    missing = sorted(expected - defined_types(text))
    if missing:
        fail(f"{path.relative_to(ROOT)} misses types: {', '.join(missing)}")


def check_no_placeholders(path: Path, text: str) -> None:
    active = []
    for line_no, line in enumerate(text.splitlines(), start=1):
        code = line.split("//", 1)[0]
        if re.search(r"\bTodo\b|\btodo!\s*\(|\bunimplemented!\s*\(|\bunimplemented_log!\s*\(", code):
            active.append(line_no)
    if active:
        fail(f"active placeholder remains in {path.relative_to(ROOT)} at lines {active}")


def variant_names(text: str) -> set[str]:
    enum_match = re.search(r"pub enum SapMsgInner\s*\{", text)
    if not enum_match:
        fail("SapMsgInner enum not found")
    start = enum_match.end()
    depth = 1
    i = start
    while i < len(text) and depth:
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
        i += 1
    body = text[start : i - 1]
    return set(re.findall(r"(?m)^\s{4}([A-Z][A-Za-z0-9_]*)\s*(?:\(|\{|,)", body))


def check_sap_variants(text: str) -> None:
    variants = variant_names(text)
    expected = EXPECTED_TLMC | EXPECTED_LTPD
    missing = sorted(expected - variants)
    if missing:
        fail(f"SapMsgInner misses variants: {', '.join(missing)}")
    if 'panic!("Unknown SapMsgInner type")' in text:
        fail("SapMsgInner Display still panics for new variants")
    if '_ => write!(f, "{self:?}")' not in text:
        fail("SapMsgInner Display has no non-panicking fallback")


def extract_struct_literals(text: str, struct_name: str) -> list[str]:
    results: list[str] = []
    pattern = re.compile(rf"\b{re.escape(struct_name)}\s*\{{")
    for match in pattern.finditer(text):
        start = match.end() - 1
        depth = 0
        i = start
        state = "code"
        while i < len(text):
            ch = text[i]
            nxt = text[i + 1] if i + 1 < len(text) else ""
            if state == "code":
                if ch == "/" and nxt == "/":
                    state = "line"
                    i += 2
                    continue
                if ch == '"':
                    state = "string"
                elif ch == "{":
                    depth += 1
                elif ch == "}":
                    depth -= 1
                    if depth == 0:
                        results.append(text[start : i + 1])
                        break
            elif state == "line":
                if ch == "\n":
                    state = "code"
            elif state == "string":
                if ch == "\\":
                    i += 2
                    continue
                if ch == '"':
                    state = "code"
            i += 1
    return results


def check_ltpd_integration(bs: str, ms: str) -> None:
    for label, text in (("mle_bs.rs", bs), ("mle_ms.rs", ms)):
        literals = extract_struct_literals(text, "LtpdMleUnitdataInd")
        if not literals:
            fail(f"no LtpdMleUnitdataInd construction found in {label}")
        for literal in literals:
            if "received_address_type:" not in literal:
                fail(f"LtpdMleUnitdataInd misses received_address_type in {label}")
    if "Sap::LcmcSap" in "\n".join(
        literal for literal in extract_struct_literals(ms, "SapMsg") if "LtpdMleUnitdataInd" in literal
    ):
        fail("MS routes an LTPD indication over LCMC")
    if re.search(r"LcmcMleUnitdataInd\s*\{[^}]*received_address_type:", ms, re.S):
        fail("received_address_type was accidentally added to LcmcMleUnitdataInd")


def main() -> int:
    files = {
        COMMON: read(COMMON),
        TLMC: read(TLMC),
        LTPD: read(LTPD),
        SAPMSG: read(SAPMSG),
        ADDRESS: read(ADDRESS),
        MLE_BS: read(MLE_BS),
        MLE_MS: read(MLE_MS),
        TESTS: read(TESTS),
    }

    for path, text in files.items():
        check_delimiters(path, text)

    check_expected_types(COMMON, files[COMMON], EXPECTED_COMMON)
    check_expected_types(TLMC, files[TLMC], EXPECTED_TLMC)
    check_expected_types(LTPD, files[LTPD], EXPECTED_LTPD)

    for path in (COMMON, TLMC, LTPD):
        check_no_placeholders(path, files[path])

    check_sap_variants(files[SAPMSG])
    check_ltpd_integration(files[MLE_BS], files[MLE_MS])

    address = files[ADDRESS]
    ssi_pos = address.find("pub enum SsiType")
    tetra_pos = address.find("pub struct TetraAddress")
    ssi_block = address[max(0, ssi_pos - 160) : ssi_pos]
    tetra_block = address[max(0, tetra_pos - 160) : tetra_pos]
    if not all(token in ssi_block for token in ("PartialEq", "Eq", "Hash")):
        fail("SsiType must implement PartialEq, Eq and Hash")
    if not all(token in tetra_block for token in ("PartialEq", "Eq", "Hash")):
        fail("TetraAddress must implement PartialEq, Eq and Hash")

    print("SWMI Foundation 1 Package B static checks passed.")
    print(f"  common types: {len(EXPECTED_COMMON)} required")
    print(f"  TLMC primitives: {len(EXPECTED_TLMC)} required")
    print(f"  LTPD primitives: {len(EXPECTED_LTPD)} required")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
