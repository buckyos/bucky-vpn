# bucky-vpn

## 角色
VPN 客户端 Rust 应用。

## 输入
- 本地客户端配置
- 用户凭据与运行时环境
- `vpn-frame` 暴露的共享服务

## 输出
- 客户端运行时行为
- `vpn-client/src/api.rs` 暴露的本地 API/服务面
- 打包后的客户端二进制

## 依赖
- `vpn-frame`
- Rust workspace 工具链
- Windows 平台上的服务与打包逻辑

## 边界
- 负责客户端 CLI、启动装配和本地控制面
- 不应吸收属于 `vpn-frame` 或 `bucky-vpn-server` 的共享协议与服务端职责
- PN 连接由客户端把同一 PN 的全部有序、去重端点放入一个 `TtpTarget`，并保存完整目标用于移除；`p2p-frame` 的 TTP 负责候选拨号、目标维护与恢复。客户端保留现有 QUIC 优先顺序，不新增拨号或重试策略。
- 根目录 `install.ps1` 和 `install.sh` 负责从 `buckyos/bucky-vpn` 最新正式 GitHub Release 选择现有平台安装包，要求 HTTPS 和资产 SHA-256 校验后调用原生安装器；支持 Windows x64、Debian/Ubuntu amd64 和 macOS 通用 PKG。Windows 使用 Inno 静默安装、等待退出并保留日志；Linux 使用 `dpkg -i`；macOS 支持本次未签名/不受信任包的安装及定向移除 Bucky VPN 产物的隔离属性。安装布局、服务注册与失败恢复仍由原生安装包负责，脚本不实现后台更新或全局安全策略修改。Unix 入口要求 Bash、curl 和 jq；使用方式由根目录 README 维护。
