#!/bin/bash
APP_NAME="BuckyVPN"
VERSION="1.2.0"
PKG_NAME="${APP_NAME}-${VERSION}.pkg"
PKG_DIR="./vpn_macos"

cargo build -p bucky-vpn --target x86_64-apple-darwin --release
cargo build -p bucky-vpn --target aarch64-apple-darwin --release

# 清理旧文件
rm -rf "${PKG_DIR}/${APP_NAME}.app/Contents/MacOS" "${PKG_NAME}"

mkdir -p "${PKG_DIR}/${APP_NAME}.app/Contents/MacOS"
mkdir -p "${PKG_DIR}/${APP_NAME}.app/Contents/Resources"

lipo -create -output "${PKG_DIR}/${APP_NAME}.app/Contents/MacOS/bucky-vpn" target/x86_64-apple-darwin/release/bucky-vpn target/aarch64-apple-darwin/release/bucky-vpn

cat > ${PKG_DIR}/${APP_NAME}.app/Contents/Info.plist <<EOF
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

pkgbuild --root ${PKG_DIR} --identifier "com.bucky.vpn" --version "${VERSION}" --install-location "/Applications" --scripts ${PKG_DIR}/scripts ./dist/${PKG_NAME}

# 清理临时文件
# rm -rf "${PKG_DIR}"
echo "build success: ./dist/${PKG_NAME}"
