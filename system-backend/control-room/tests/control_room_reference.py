#!/usr/bin/env python3
"""Dependency, incident and federated-summary reference model for Control Room."""
from dataclasses import dataclass


@dataclass
class Service:
    name: str
    critical: bool
    failures: int = 0
    status: str = "unknown"


def apply(service: Service, live: bool, ready: bool, threshold: int = 3) -> str | None:
    service.status = "healthy" if live and ready else "degraded" if live else "offline"
    service.failures = 0 if service.status == "healthy" else service.failures + 1
    if service.status == "offline" and service.failures >= threshold:
        return f"service:{service.name}"
    return None


def first_metric(summaries: dict[str, dict], candidates: list[tuple[str, str]]) -> int | None:
    for service, field in candidates:
        value = summaries.get(service, {}).get(field)
        if isinstance(value, int) and value >= 0:
            return value
    return None


def sum_metrics(summaries: dict[str, dict], candidates: list[tuple[str, str]]) -> int | None:
    values = [summaries.get(service, {}).get(field) for service, field in candidates]
    values = [value for value in values if isinstance(value, int) and value >= 0]
    return sum(values) if values else None


def main() -> None:
    service = Service("call-control", True)
    assert apply(service, False, False) is None
    assert apply(service, False, False) is None
    assert apply(service, False, False) == "service:call-control"
    assert service.status == "offline"
    assert apply(service, True, True) is None
    assert service.failures == 0

    summaries = {
        "node-gateway": {"connected_nodes": 3},
        "subscriber-core": {"observed_registered": 41},
        "call-control": {"calls_active": 2},
        "sds-router": {"queued": 4, "offline": 3, "in_flight": 1},
    }
    assert first_metric(summaries, [("node-gateway", "connected_nodes")]) == 3
    assert first_metric(summaries, [("subscriber-core", "observed_registered")]) == 41
    assert first_metric(summaries, [("call-control", "calls_active")]) == 2
    assert sum_metrics(summaries, [("sds-router", "queued"), ("sds-router", "offline"), ("sds-router", "in_flight")]) == 8
    assert first_metric(summaries, [("packet-core", "contexts_ready")]) is None
    print("Control Room reference model: OK")


if __name__ == "__main__":
    main()
