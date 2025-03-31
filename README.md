BuckyVPN是一个p2p实现的VPN工具，现在支持Windows、Debian/Ubuntu、MacOS平台。

### 编译：

1. 执行build_server.sh将服务端打包成一个docker镜像
2. 在windows上执行build_win.bat打包BuckyVPN客户端的windows安装包，打包过程依赖于Inno打包工具
3. 在Debian/Ubuntu上执行build_deb.sh将打包BuckyVPN客户端的deb安装包
4. 在MacOS上执行build_macos.sh将打包BuckyVPN客户端的MacOS安装包

### 服务端部署：

```sh
docker run --rm -d -p 8888:80 -p 3424:3424/udp --name vpn-server \
-e VPN_ADMIN_NAME=服务端管理用户名 \
-e VPN_ADMIN_PASSWORD=服务端管理用户密码 \
-e VPN_JWT_KEY=服务端登录jwt token密码，可任意字符串 \
-e VPN_DATA_DIR=/bucky_vpn_data \
-v ./test_data:/bucky_vpn_data bucky-vpn-server:latest
```

### 客户端加入服务器：

可通过bucky-vpn join命令加入指定服务器

```
bucky-vpn join -h
join a vpn network

Usage: bucky-vpn join [OPTIONS] --server <server> --server_id <server_id> --network_id <network_id>

Options:
  -s, --server <server>          The vpn server ip or domain
  -p, --port <port>              The vpn server port
      --server_id <server_id>    The vpn server identity ID
      --network_id <network_id>  The network id you want to join
  -n, --name <name>              The name of the node seen on the server
  -h, --help                     Print help
```

