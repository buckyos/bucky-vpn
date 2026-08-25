#! /bin/bash

docker run --rm -d -p 8888:80 -p 3424:3424/udp --name vpn-server \
-e VPN_ADMIN_NAME=wugren \
-e VPN_ADMIN_PASSWORD=123456 \
-e VPN_JWT_KEY=dkflseosoidfsjdkflsdsfs \
-e VPN_DATA_DIR=/test_data \
-v /root/work/vpn/test_data:/test_data vpn-server:latest

