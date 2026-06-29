---
module: vpn-frame
version: v0.1
doc: client-runtime
status: draft
---

# vpn-frame Client Runtime Design

## 目标
- 说明 `vpn-frame/src/client/` 的运行时分层、主流程和状态边界。
- 让后续客户端相关实现能快速定位配置同步、隧道收发和设备生命周期逻辑。
- 为共享授权合同的消费方提供客户端视角的依赖说明。

## 范围
### In scope
- `vpn_client.rs`
- `vpn_server_client.rs`
- `tunnel_manager.rs`
- `packet_dispatcher.rs`
- `vpn_device.rs`
- `vpn_client_manager.rs`

### Out of scope
- `vpn-client` 二进制中的 CLI、配置文件装配和 HTTP 本地 API
- 服务端 store、节点授权和网络成员真相
- `tun-rs` 和底层 P2P 网络实现细节

## 总体职责
客户端运行时负责三件事：

1. 与服务端交换控制面命令，拉取当前节点可见的网络拓扑。
2. 为每个可见网络创建或更新本地虚拟设备，并把设备流量分发到远端节点。
3. 接收远端隧道数据并回注到对应虚拟设备。

客户端本身不决定授权结果；它只消费服务端返回的可见网络和成员列表。

## 结构拆分
| 路径 | 职责 | 关键类型 |
|------|------|----------|
| `client/vpn_client.rs` | 客户端主协调器，周期同步服务端拓扑并维护本地网络设备 | `VpnClient` |
| `client/vpn_server_client.rs` | 服务端命令封装，处理 `JoinNetworkGroup`、`GetVpnInfo`、`QueryNode` | `VpnServerClient` |
| `client/tunnel_manager.rs` | 维护目标节点到 tunnel worker 的映射，并处理入站 tunnel 数据 | `TunnelManager`, `VpnRouter` |
| `client/packet_dispatcher.rs` | 把设备出口流量按目标节点和流拆分、排队、批量发送 | `PacketDispatcher` |
| `client/vpn_device.rs` | 创建和维护 tun 设备，收包后交给上层回调 | `VpnDevice` |
| `client/vpn_client_manager.rs` | 以 key 缓存多个 `VpnClient` 实例 | `VpnClientManager` |

## 主流程
### 1. 控制面同步
- `VpnClient::run` 启动后台循环。
- 首次同步调用 `VpnServerClient::get_vpn_info(None, Some(client_version))`。
- 后续同步带上当前 `info_version`，只在服务端版本变化时重建本地视图。
- 服务端返回 `NodeVpnInfo` 列表，客户端据此重建设备和路由信息。

### 2. 设备生命周期
- 每个 `NodeNetwork` 对应一个 `VpnDevice`。
- `VpnDevice::create_device` 根据 IPv4/IPv6 地址段创建 tun 设备。
- `VpnDevice::start` 启动收包循环，验证目标 IP 是否仍在当前网段内。
- 当网络配置变化时，`VpnDevice::update_device` 会重建底层设备并恢复接收回调。

### 3. 出站数据
- 设备收到 IP 包后，通过 `PacketRecv::on_recv` 回调进入 `PacketDispatcher`。
- `PacketDispatcher` 根据 `(group_id, network_id, target_ip)` 选择目标 dispatcher。
- 每个目标 dispatcher 维护分片队列，按 flow hash 维持相对稳定的发送顺序。
- 发送失败时按指数退避重新获取 tunnel sender。

### 4. 入站数据
- `TunnelManager` 在构造时启动 listener accept 循环，收下远端入站 tunnel。
- 每条 tunnel 上的 `DataHeader` 会被解析出 `network_id` 和负载长度。
- 负载经 `TunnelPkgRecv` 回调交回 `VpnClient`。
- `VpnClient` 根据 `network_id` 找到对应 `VpnDevice`，再写回本地虚拟网卡。

## 状态与缓存
### `VpnClient`
- 持有当前设备集合 `vpn_devices`。
- 持有 `TunnelManager` 与 `PacketDispatcher`。
- 通过 `cur_version` 和 `is_first` 控制增量同步。

### `TunnelManager`
- `VpnRouter` 保存 `(group_id, network_id, ip) -> node_id` 路由。
- `PendingSendCache` 暂存刚接入、尚未被目标 worker 绑定的入站 sender。
- `WorkerPool` 复用到同一目标节点的 tunnel sender。

### `PacketDispatcher`
- 按目标维度缓存 `TargetDispatcher`。
- 每个目标再按 shard 拆分异步队列，减少单队列阻塞。

## 与共享授权合同的关系
- 客户端只消费服务端返回的 `NodeVpnInfo.members` 和 `QueryNodeResp.node_id`。
- 当服务端开始拒绝未授权或跨组请求时，客户端会表现为：
  - `GetVpnInfo` 返回错误，客户端本轮同步失败并等待下一轮重试。
  - `QueryNode` 返回 `None`，对应目标无法建立新 tunnel。
- 客户端不应自行推断“哪些节点本该可见”，而应完全信任服务端返回的可见集合。

## 关键约束
- 所有运行时循环保持 Tokio 异步风格，不在 hot path 中执行阻塞操作。
- `VpnDevice` 只处理属于当前网段的 IP 包，避免越界转发。
- `PacketDispatcher` 需要在丢 tunnel 后自动重连，但必须带退避。
- 客户端运行时不持久化账号或审批真相，只缓存服务端下发的即时视图。

## 风险点
- 服务端拓扑频繁变化时，设备重建和路由刷新可能带来短暂抖动。
- 若 `QueryNode` 可见性与 `members` 列表不一致，可能出现“路由存在但无法建 tunnel”的状态。
- 若 listener 接收循环退出，入站 tunnel 将整体不可用。
