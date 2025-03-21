#! /bin/bash

cargo build -p bucky-vpn-server --target x86_64-unknown-linux-musl --release
cd vpn_web
flutter build web
cd ..

docker build -t bucky-vpn-server:latest . -f Dockerfile
