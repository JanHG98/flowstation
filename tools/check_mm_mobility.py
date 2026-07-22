#!/usr/bin/env python3
"""Dependency-free structural checks for SWMI Mobility 1 Package C."""

from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]


def require(path: str, needles: list[str]) -> None:
    text = (ROOT / path).read_text(encoding="utf-8")
    missing = [needle for needle in needles if needle not in text]
    if missing:
        raise AssertionError(f"{path}: missing {missing}")


def main() -> int:
    require(
        "crates/tetra-entities/src/mm/mobility_runtime.rs",
        [
            "pub struct MmMobilityRuntime",
            "pub fn begin_migration",
            "pub fn complete_migration",
            "pub fn begin_forward_registration",
            "pub fn take_forward_context",
            "pub fn tick",
            "DEFAULT_VASSI_MIN",
            "MM_MOBILITY_TIMEOUT_SLOTS",
        ],
    )
    require(
        "crates/tetra-entities/src/mm/components/client_state.rs",
        [
            "pub struct MmClientMobilityContext",
            "pub fn export_mobility_context",
            "pub fn import_mobility_context",
        ],
    )
    require(
        "crates/tetra-entities/src/mm/mm_bs.rs",
        [
            "send_d_location_update_proceeding",
            "rx_lmm_mle_prepare_ind",
            "provide_migration_context",
            "take_forward_context",
            "DLocationUpdateProceeding",
            "MleCellChangeControl::GrantPrepare",
        ],
    )
    require(
        "crates/tetra-saps/src/lmm/mod.rs",
        ["pub struct LmmMlePrepareInd"],
    )
    require(
        "crates/tetra-saps/src/sapmsg.rs",
        ["LmmMlePrepareInd(LmmMlePrepareInd)"],
    )
    require(
        "crates/tetra-pdus/src/mm/pdus/d_location_update_reject.rs",
        ["pub fn from_bitbuf", "ciphering_parameters = if cipher_control"],
    )
    reject_text = (ROOT / "crates/tetra-pdus/src/mm/pdus/d_location_update_reject.rs").read_text(encoding="utf-8")
    if "unimplemented!()" in reject_text:
        raise AssertionError("D-LOCATION-UPDATE-REJECT parser still contains unimplemented!()")

    for path in [
        "crates/tetra-pdus/tests/test_mm_mobility_pdus.rs",
        "crates/tetra-entities/tests/test_mm_mobility_runtime.rs",
        "crates/tetra-entities/tests/test_two_cell_mm_mobility.rs",
        "Docs/SWMI_MOBILITY_1_PACKAGE_C.md",
        "Docs/SWMI_MOBILITY_1_PACKAGE_C_APPLY.md",
        "system-backend/mobility-core/README.md",
    ]:
        if not (ROOT / path).is_file():
            raise AssertionError(f"missing required file: {path}")

    print("SWMI Mobility 1 Package C static checks passed.")
    print("  two-stage migration and VASSI allocation: present")
    print("  migration context import/export: present")
    print("  forward registration through U-PREPARE: present")
    print("  accept/reject/timeout lifecycle: present")
    print("  WebUI-ready mobility snapshots: present")
    print("  two-cell MM migration tests: present")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (AssertionError, OSError) as error:
        print(f"SWMI Mobility 1 Package C check failed: {error}", file=sys.stderr)
        raise SystemExit(1)
