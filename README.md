# BuckyVPN

BuckyVPN 是一个基于 P2P 网络实现的 VPN 工具，提供 Windows、Debian/Ubuntu 和 macOS 客户端，以及可通过 Docker 部署的服务端和 Web 管理界面。

## 安装客户端

当前发布流程会把客户端安装包上传到 [GitHub Releases](https://github.com/buckyos/bucky-vpn/releases)。匹配版本标签成功发布后，可从 [Latest Release](https://github.com/buckyos/bucky-vpn/releases/latest) 下载最新版本；私有仓库需要先登录有权访问该仓库的 GitHub 账号。

| 平台 | Release 文件 |
| --- | --- |
| Windows x86-64 | `BuckyVPN_<version>_amd64_Setup.exe` |
| Debian/Ubuntu x86-64 | `bucky-vpn_<version>_amd64.deb` |
| macOS Intel/Apple Silicon | `BuckyVPN-<version>.pkg` |

其中 `<version>` 是 Release 版本号，例如 `1.2.0`。

### 使用脚本安装最新版本

无需克隆仓库：以下命令从本项目 GitHub `master` 分支下载脚本，然后查询最新正式 Release，精确选择安装包并验证 Release 资产提供的 SHA-256。要求能够匿名访问官方仓库和 GitHub 下载服务；API 限流、缺少资产或摘要、校验失败都会停止安装。摘要用于下载完整性校验，不是独立发布签名。

**Windows x64**：在“以管理员身份运行”的 PowerShell 5.1 或更新版本中执行：

```powershell
& {
    $script = Join-Path ([IO.Path]::GetTempPath()) ("bucky-vpn-" + [Guid]::NewGuid().ToString('N') + ".ps1")
    try {
        [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
        Invoke-WebRequest -UseBasicParsing -Uri 'https://raw.githubusercontent.com/buckyos/bucky-vpn/master/install.ps1' -OutFile $script -ErrorAction Stop
        & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $script
    }
    finally { Remove-Item -LiteralPath $script -Force -ErrorAction SilentlyContinue }
}
```

脚本等待 EXE 静默安装结束，不自动重启。退出码 `0` 表示完成，`3010` 表示安装完成但需要重启；其他非零值表示失败，可在 `$LASTEXITCODE` 查看。脚本会显示保留的安装日志路径（位于用户临时目录），并删除下载的安装包。仅支持 Windows x64，不支持 Windows ARM64 或 x86。

**Debian/Ubuntu amd64**：需要 Bash、curl、jq、dpkg 和 sha256sum。缺少 curl/jq 时先运行 `sudo apt-get update && sudo apt-get install -y curl jq`，然后执行：

```bash
curl --fail --show-error --location --proto '=https' --proto-redir '=https' \
  'https://raw.githubusercontent.com/buckyos/bucky-vpn/master/install.sh' -o install.sh &&
bash install.sh
```

脚本验证安装包后使用 `sudo dpkg -i`（root 会直接调用 dpkg）。需要 sudo 权限；依赖或服务环境不满足时保留 dpkg 原生错误，不自动修复依赖。不支持 RPM 发行版或 Linux ARM64。

**macOS Intel / Apple Silicon**：需要 Bash 3.2 或更新版本、curl 和 jq；curl/Bash 为系统自带，jq 需预先安装（已有 Homebrew 时可运行 `brew install jq`）。执行：

```bash
curl --fail --show-error --location --proto '=https' --proto-redir '=https' \
  'https://raw.githubusercontent.com/buckyos/bucky-vpn/master/install.sh' -o install.sh &&
bash install.sh
```

macOS 安装需要 sudo 权限。当前 PKG 未签名，脚本在 HTTPS 下载和 SHA-256 校验后调用 `sudo /usr/sbin/installer -allowUntrusted -pkg ... -target /`，并按需仅移除本次 PKG 和安装成功后的 `/Applications/BuckyVPN.app` 内的 `com.apple.quarantine` 属性，不跟随符号链接、不修改全局 Gatekeeper 设置。该选项允许不受信任证书的安装，不保证绕过 MDM 或其他系统策略。安装后隔离属性处理失败也会返回失败并说明安装已完成；原生安装日志可查看 `/var/log/install.log`。

Unix 脚本自动清理下载临时文件，保留原生安装命令的失败退出码。三个平台都沿用安装包的服务注册和启动行为，安装失败后的系统恢复由原生包管理器负责。以下仍可使用手动下载方式。

### Windows

下载并运行 `BuckyVPN_<version>_amd64_Setup.exe`，按安装向导完成安装。安装程序会注册并启动 `BuckyVPN` 系统服务，同时将安装目录加入系统 `PATH`。安装后请打开一个新的终端窗口再使用 `bucky-vpn` 命令。

### Debian/Ubuntu

下载 `.deb` 文件后，在文件所在目录执行：

```bash
VERSION=1.2.0
sudo apt install "./bucky-vpn_${VERSION}_amd64.deb"
```

安装包会注册、启用并启动 `bucky-vpn.service`。可通过以下命令查看状态和日志：

```bash
sudo systemctl status bucky-vpn
sudo journalctl -u bucky-vpn -f
```

### macOS

双击 `BuckyVPN-<version>.pkg` 并按安装向导操作，或者在终端执行：

```bash
VERSION=1.2.0
sudo installer -pkg "./BuckyVPN-${VERSION}.pkg" -target /
```

程序安装在 `/Applications/BuckyVPN.app`，后台服务由安装包注册。若当前终端尚未识别 `bucky-vpn`，可直接使用：

```bash
/Applications/BuckyVPN.app/Contents/MacOS/bucky-vpn --help
```

## 部署服务端

当前发布流程会把服务端镜像上传到 GitHub Container Registry：

- `ghcr.io/buckyos/bucky-vpn-server:<version>`：固定版本镜像，推荐生产部署使用。
- `ghcr.io/buckyos/bucky-vpn-server:latest`：最近一次成功发布时更新的滚动标签。

首先准备配置和数据目录。下面是一份可以直接启动控制节点和内置代理节点的最小配置；完整选项见 [`vpn-server/config/config.example.yaml`](vpn-server/config/config.example.yaml)：

```bash
mkdir -p bucky-vpn-server/data
cd bucky-vpn-server

cat > config.yaml <<'YAML'
name: "bucky-vpn-server"
ip: "0.0.0.0"
port: 3624

sn:
  enabled: true
  http:
    ip: "0.0.0.0"
    port: 3445
  admin:
    name: "admin"
    password: "change-me"
  jwt:
    key: "change-me-to-a-long-random-secret"

pn:
  enabled: true
  # advertised_ip: "203.0.113.20"
  # port_mapping:
  #   quic: 3624
  #   tcp: 3624

data:
  dir: "/bucky-vpn/data"

log: true
log.level: "info"
YAML
```

部署前至少需要修改 `config.yaml` 中的以下配置：

- `sn.admin.name` 和 `sn.admin.password`：Web 管理员账号和密码。
- `sn.jwt.key`：替换为足够长的随机签名密钥。
- `pn.advertised_ip`：服务端位于 NAT 后或自动探测地址不正确时，填写客户端可访问的公网 IP。
- `pn.port_mapping`：公网端口与容器内 `3624` 不同时，填写实际映射的 TCP/QUIC 端口。

如果 GHCR 包不是公开可读，请先使用具有 `read:packages` 权限的 GitHub Token 登录：

```bash
GITHUB_USER=your-github-user
echo "$GHCR_TOKEN" | docker login ghcr.io -u "$GITHUB_USER" --password-stdin
```

然后启动容器：

```bash
docker pull ghcr.io/buckyos/bucky-vpn-server:latest

docker run -d \
  --name bucky-vpn-server \
  --restart unless-stopped \
  -p 80:80 \
  -p 3624:3624/tcp \
  -p 3624:3624/udp \
  -e VPN_DATA_DIR=/bucky-vpn/data \
  -v "$PWD/config.yaml:/bucky-vpn/config.yaml:ro" \
  -v "$PWD/data:/bucky-vpn/data" \
  ghcr.io/buckyos/bucky-vpn-server:latest
```

容器端口用途：

- `80/tcp`：Web 管理界面和 `/api/` 接口。
- `3624/tcp`：P2P TCP 通道。
- `3624/udp`：P2P QUIC 通道。

部署固定版本时，将上面两处 `latest` 替换为相同的版本号。若主机启用了防火墙，还需要放行 Web 访问端口以及 P2P 的 TCP/UDP 端口。

## 客户端加入 VPN 网络

客户端安装并启动后台服务后，可以使用 `join` 命令加入服务端上的网络：

```bash
SERVER_ID=replace-with-server-identity-id

bucky-vpn join \
  --server vpn.example.com \
  --port 3624 \
  --server_id "$SERVER_ID" \
  --network_id 1 \
  --name my-client
```

必填参数：

- `--server`：服务端 IP 或域名。
- `--server_id`：服务端 identity ID。
- `--network_id`：要加入的网络 ID。

`--port` 省略时默认为 `3624`。使用域名连接但域名与服务端证书名不一致时，可通过 `--server_name` 显式指定服务端名称。完整参数请运行 `bucky-vpn join --help` 查看。

## Release 与构建产物

仓库的 `Build and release` GitHub Actions 工作流支持两种入口：

- 手动运行：构建 Debian、macOS、Windows 安装包和服务端镜像；三个客户端安装包作为 Actions Artifacts 保留 14 天，服务端镜像只在 runner 内完成构建验证，不发布 GitHub Release 或 GHCR 镜像。
- 推送版本标签：仅当标签严格等于 `v<vpn-client Cargo version>` 且仓库为 `buckyos/bucky-vpn` 时，才发布 GitHub Release 和 GHCR 镜像。

正式发布包含：

- GitHub Release：`.deb`、`.pkg` 和 Windows `.exe` 三个客户端安装包，以及 GitHub 自动生成的源码归档。
- GHCR：同一个服务端镜像的 `<version>` 和 `latest` 两个标签。

发布版本的唯一来源是 [`vpn-client/Cargo.toml`](vpn-client/Cargo.toml) 中的 `package.version`。

## 从源码构建

各平台构建入口如下：

| 目标 | 命令 | 输出 |
| --- | --- | --- |
| Debian/Ubuntu 客户端 | `./build_deb.sh` | `dist/bucky-vpn_<version>_amd64.deb` |
| macOS 通用客户端 | `./build_macos.sh` | `dist/BuckyVPN-<version>.pkg` |
| Windows 客户端 | `build_win.bat` | `dist/BuckyVPN_<version>_amd64_Setup.exe` |
| 服务端与 Web UI | `./build_server.sh` | 本地镜像 `bucky-vpn-server:latest` |

这些脚本需要对应平台的 Rust 工具链和打包工具。CI 使用的 runner、Rust target、Flutter、NASM 和 Inno Setup 配置可参考 [`.github/workflows/build.yml`](.github/workflows/build.yml)。
