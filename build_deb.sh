#! /bin/bash

cargo build -p bucky-vpn --target x86_64-unknown-linux-musl --release
mkdir -p vpn_deb/usr/bin
cp target/x86_64-unknown-linux-musl/release/bucky-vpn ./vpn_deb/usr/bin/bucky-vpn
chmod +x vpn_deb/usr/bin/bucky-vpn

mkdir -p ./dist
dpkg-deb -b vpn_deb ./dist/bucky-vpn_1.0.0_amd64.deb
