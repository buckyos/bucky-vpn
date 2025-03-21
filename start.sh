#! /bin/sh

# 启动后台服务
echo "Starting background services..."

/usr/share/bucky-vpn-server &

# 启动 Nginx
echo "Starting Nginx..."
nginx -g "daemon off;"
