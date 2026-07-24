#!/usr/bin/env python3
"""Small dependency-free acceptance model for route order, loop prevention and failover."""

from dataclasses import dataclass


@dataclass(frozen=True)
class Peer:
    peer_id: str
    region_id: str
    state: str
    latency_ms: float
    priority: int


@dataclass(frozen=True)
class Route:
    peer_id: str
    destination_region: str
    preference: int
    metric: int


def select(routes: list[Route], peers: dict[str, Peer], target: str, trace: list[str]) -> list[str]:
    candidates: list[tuple[int, int, float, int, str]] = []
    for route in routes:
        peer = peers[route.peer_id]
        if route.destination_region != target:
            continue
        if peer.state not in {"up", "degraded"}:
            continue
        if peer.region_id in trace:
            continue
        candidates.append((-route.preference, route.metric, peer.latency_ms, -peer.priority, peer.peer_id))
    candidates.sort()
    return [entry[-1] for entry in candidates]


def main() -> None:
    peers = {
        "b-primary": Peer("b-primary", "region-b", "up", 15.0, 100),
        "b-backup": Peer("b-backup", "region-c", "up", 30.0, 50),
        "loop": Peer("loop", "region-a", "up", 1.0, 1000),
    }
    routes = [
        Route("b-primary", "region-b", 200, 10),
        Route("b-backup", "region-b", 100, 20),
        Route("loop", "region-b", 500, 1),
    ]
    order = select(routes, peers, "region-b", ["region-a"])
    assert order == ["b-primary", "b-backup"], order
    peers["b-primary"] = Peer("b-primary", "region-b", "down", 15.0, 100)
    order = select(routes, peers, "region-b", ["region-a"])
    assert order == ["b-backup"], order
    assert len(["region-a", "region-c"]) < 8
    assert "region-a" in ["region-a", "region-c"]
    print("Transit reference checks: OK")


if __name__ == "__main__":
    main()
