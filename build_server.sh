#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

VERSION="$({ cargo metadata --no-deps --format-version 1; } | python3 -c '
import json
import re
import sys

packages = [item for item in json.load(sys.stdin)["packages"] if item["name"] == "bucky-vpn"]
if len(packages) != 1:
    raise SystemExit("expected exactly one bucky-vpn package in cargo metadata")
version = packages[0]["version"]
if re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version) is None:
    raise SystemExit(f"unsupported installer version: {version}")
print(version)
')"

cargo build -p bucky-vpn-server --target x86_64-unknown-linux-musl --release
(cd vpn_web && flutter build web)

docker build --build-arg "VERSION=${VERSION}" -t bucky-vpn-server:latest . -f Dockerfile
