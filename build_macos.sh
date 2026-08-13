#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

APP_NAME="BuckyVPN"
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
PKG_NAME="${APP_NAME}-${VERSION}.pkg"
PKG_DIR="./vpn_macos"

cargo build -p bucky-vpn --target x86_64-apple-darwin --release
cargo build -p bucky-vpn --target aarch64-apple-darwin --release

PACKAGE_TEMP="$(mktemp -d "${TMPDIR:-/tmp}/bucky-vpn-macos.XXXXXX")"
trap 'rm -rf "$PACKAGE_TEMP"' EXIT
PACKAGE_ROOT="$PACKAGE_TEMP/root"
mkdir -p "$PACKAGE_ROOT"
cp -a "${PKG_DIR}/${APP_NAME}.app" "$PACKAGE_ROOT/"
mkdir -p "$PACKAGE_ROOT/${APP_NAME}.app/Contents/MacOS"
mkdir -p "$PACKAGE_ROOT/${APP_NAME}.app/Contents/Resources"

lipo -create -output "$PACKAGE_ROOT/${APP_NAME}.app/Contents/MacOS/bucky-vpn" target/x86_64-apple-darwin/release/bucky-vpn target/aarch64-apple-darwin/release/bucky-vpn

cat > "$PACKAGE_ROOT/${APP_NAME}.app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>bucky-vpn</string>
    <key>CFBundleIdentifier</key>
    <string>com.bucky.${APP_NAME}</string>
    <key>CFBundleVersion</key>
    <string>${VERSION}</string>
    <key>CFBundleIconFile</key>
    <string>icon.icns</string>
</dict>

</plist>
EOF

mkdir -p ./dist

pkgbuild --root "$PACKAGE_ROOT" --identifier "com.bucky.vpn" --version "${VERSION}" --install-location "/Applications" --scripts "${PKG_DIR}/scripts" "./dist/${PKG_NAME}"

echo "build success: ./dist/${PKG_NAME}"
