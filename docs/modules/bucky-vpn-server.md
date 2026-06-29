# bucky-vpn-server

## 角色
VPN 服务端 Rust 应用，负责把账户体系、SQLite 持久化、P2P 基础设施、共享 VPN 运行时、PN relay 鉴权和 HTTP 控制面装配成可部署进程。

## 输入
- 服务端配置文件与 `VPN_` 环境变量
- 本地数据目录中的 SQLite 数据库和 identity 文件
- `vpn-frame` 提供的共享服务端运行时合同
- `p2p-frame` 提供的 X509 identity、SN service 和 PN server

## 输出
- 服务端运行时行为
- 账户接口和 VPN 管理 HTTP API
- 受账号组和审批状态约束的 PN relay 连接能力
- 可部署的服务端二进制与数据目录状态

## 依赖
- `vpn-frame`
- `p2p-frame`
- `sfo-account`
- `sfo-http`
- `sfo-sql`
- `vpn-server/Cargo.toml` 中声明的其余装配依赖

## 边界
- 负责服务端装配、服务端专属持久化和服务端控制面
- 通过 `SqliteUserStore` 维护账号到网络组的绑定，通过 SQLite store 维护节点、网络和成员关系
- 负责 PN relay 连接期的 source-target 授权，不把这部分责任下放给客户端自觉或外部 relay 默认行为
- 不应吸收客户端 UI 逻辑、共享协议定义或外部 P2P 组件内部实现
