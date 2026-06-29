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
