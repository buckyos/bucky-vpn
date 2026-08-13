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

cargo build -p bucky-vpn --target x86_64-unknown-linux-musl --release

PACKAGE_TEMP="$(mktemp -d "${TMPDIR:-/tmp}/bucky-vpn-deb.XXXXXX")"
trap 'rm -rf "$PACKAGE_TEMP"' EXIT
PACKAGE_ROOT="$PACKAGE_TEMP/root"
mkdir -p "$PACKAGE_ROOT"
cp -a vpn_deb/. "$PACKAGE_ROOT/"

# Repository files can appear as 0777 on Windows-backed worktrees. Normalize the
# staged package so dpkg-deb receives valid Debian control and payload modes.
find "$PACKAGE_ROOT" -type d -exec chmod 0755 {} +
find "$PACKAGE_ROOT" -type f -exec chmod 0644 {} +
for maintainer_script in preinst postinst prerm postrm; do
    if [[ -f "$PACKAGE_ROOT/DEBIAN/$maintainer_script" ]]; then
        chmod 0755 "$PACKAGE_ROOT/DEBIAN/$maintainer_script"
    fi
done

install -Dm755 target/x86_64-unknown-linux-musl/release/bucky-vpn "$PACKAGE_ROOT/usr/bin/bucky-vpn"
sed -i "s/^Version:.*/Version: ${VERSION}/" "$PACKAGE_ROOT/DEBIAN/control"
grep -Fx "Version: ${VERSION}" "$PACKAGE_ROOT/DEBIAN/control" >/dev/null

mkdir -p dist
dpkg-deb --root-owner-group --build "$PACKAGE_ROOT" "dist/bucky-vpn_${VERSION}_amd64.deb"
