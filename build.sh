#! /bin/bash

cargo build -p vpn-server --target x86_64-unknown-linux-musl --release
cd vpn_web
flutter build web
cd ..

docker build -t vpn-server:latest . -f Dockerfile
