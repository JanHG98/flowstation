#!/usr/bin/env python3
"""Berechnet ausschließlich für den Open-Lab-Provider eine Testantwort."""
import argparse
import hashlib
import hmac
from pathlib import Path


def u32(value: int) -> bytes:
    return value.to_bytes(4, "big")


def part(value: bytes) -> bytes:
    return u32(len(value)) + value


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--seed", type=Path, required=True)
    parser.add_argument("--issi", type=int, required=True)
    parser.add_argument("--node", required=True)
    parser.add_argument("--context", required=True)
    parser.add_argument("--challenge", required=True, help="hex")
    parser.add_argument("--bytes", type=int, default=16)
    args = parser.parse_args()
    seed = args.seed.read_bytes()
    subscriber = hmac.new(
        seed,
        b"netcore-security-core/lab-subscriber/v1" + u32(args.issi),
        hashlib.sha256,
    ).digest()
    challenge = bytes.fromhex(args.challenge)
    payload = (
        b"netcore-security-core/lab-response/v1"
        + u32(args.issi)
        + part(args.node.encode())
        + part(args.context.encode())
        + part(challenge)
    )
    print(hmac.new(subscriber, payload, hashlib.sha256).digest()[: args.bytes].hex())


if __name__ == "__main__":
    main()
