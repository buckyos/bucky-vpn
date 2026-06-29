---
module: vpn-frame
version: v0.1
doc: server-runtime
status: draft
---

# vpn-frame Server Runtime Design

## 目标
- 说明 `vpn-frame/src/server/` 的运行时分层、主流程和状态边界。
- 明确服务端如何把节点加入、网络成员、在线状态和授权合同组合成控制面结果。
- 为 `bucky-vpn-server` 装配层与共享服务端运行时之间的责任分界提供稳定说明。

## 范围
### In scope
- `server/vpn_server.rs`
- `server/network_manager.rs`
- `server/network_store.rs`
- `server/node_manager.rs`
- `server/node_store.rs`
- `server/vpn_store.rs`

### Out of scope
- `bucky-vpn-server` 中的 HTTP API、SQLite 具体实现和管理员初始化
- 外部 `p2p-frame` / `bucky-p2p` 的 PN relay 桥接实现
- 客户端 tun 设备和本地包转发细节

## 总体职责
服务端运行时负责四类能力：

1. 接收客户端的控制命令。
2. 维护节点在线状态和信息版本。
3. 基于 store 真相计算节点可见网络和成员列表。
4. 根据网络组、成员关系和授权状态回答目标节点解析请求。

共享服务端运行时不拥有账号真相；它依赖外部装配层提供的 `VpnStore` 实现。

## 结构拆分
| 路径 | 职责 | 关键类型 |
|------|------|----------|
| `server/vpn_server.rs` | 控制命令入口、在线节点缓存和主流程编排 | `VpnServer`, `OnlineNode` |
| `server/network_manager.rs` | 网络组、成员、审批状态和拓扑刷新逻辑 | `NetworkManager` |
| `server/network_store.rs` | 网络相关存储合同 | `NetworkStore`, `JoinedNode`, `Network`, `NetworkMember` |
| `server/node_manager.rs` | 节点缓存和 `Node` 读取 | `NodeManager` |
| `server/node_store.rs` | 节点 ID/节点元信息存储合同 | `NodeStore`, `NodeId`, `Node` |
| `server/vpn_store.rs` | 聚合 store 合同和事务 guard | `VpnStore`, `VpnStoreGuard`, `VpnStoreFactory` |

## 主流程
### 1. 运行时启动
- `VpnServer::new` 创建 `NodeManager` 和 `NetworkManager`。
- `VpnServer::start` 注册命令处理器，并启动离线节点监视循环。
- 离线监视循环每 65 秒扫描一次 `OnlineNodesState`，把过期节点回写到 `NetworkManager::node_offline`。

### 2. 节点加入网络组
- 客户端发送 `JoinNetworkGroupReq`。
- `vpn_server.rs` 校验目标网络组是否存在。
- 未加入过的节点会通过 `NetworkManager::add_joined_node` 写入 store。
- 新加入节点默认 `allow_join = false`，需要外部装配层审批后才可见。

### 3. 获取 VPN 拓扑
- 客户端发送 `GetVpnInfoReq`。
- 服务端先更新在线节点时间戳和客户端版本信息。
- `handle_get_vpn_info_req` 通过 `NetworkManager::get_networks_of_node` 拉取当前节点可见的网络视图。
- 对每个网络，再通过 `get_allowed_network_member` 过滤出可见成员，并结合 `online_nodes` 只返回在线目标。
- 返回结果以 `info_version` 控制增量同步。

### 4. 查询目标节点
- 客户端发送 `QueryNodeReq`。
- 服务端依据网络组、网络 ID 和目标 IP 返回对应 `NodeId`。
- 授权逻辑要求请求节点和目标节点都满足当前网络组下的可见性约束。

## 数据与状态
### `NodeId` / `Node`
- `NodeId` 是共享的节点标识，支持 base58/base36 转换。
- `Node.info_version` 用于驱动客户端拓扑增量刷新。

### `JoinedNode`
- 表示节点已申请加入某个网络组。
- `allow_join` 是设备级授权开关。
- `name` 和 `comment` 作为管理元数据，不改变授权边界。

### `Network` / `NetworkMember`
- `Network` 定义逻辑网段和掩码。
- `NetworkMember` 把节点映射到具体 IP / IPv6 地址。
- 节点是否属于某网络，不仅取决于 `network_member`，还取决于所属网络组和审批状态。

### `OnlineNodesState`
- 维护双缓存在线表，减少扫描时的整表拷贝。
- 节点超过 120 秒未刷新会被视为过期。

## 授权边界
- 账号边界通过外部 store 的 `group_id` 投影进入共享运行时。
- 设备边界通过 `JoinedNode.allow_join` 进入共享运行时。
- 共享服务端运行时只暴露“同组且已允许”的网络和节点。
- 现阶段该授权边界主要约束控制面：
  - `GetVpnInfo`
  - `QueryNode`
- 外部 PN relay 在 source-target 桥接层仍没有独立授权钩子，因此 relay 级硬封堵不在本模块内完成。

## 一致性策略
- `NetworkManager` 在更新网络、成员或审批状态时，会递增相关节点的 `info_version`。
- 同时通过 `NodeManager::remove_node` 让缓存失效，迫使后续读取看到最新状态。
- 事务由 `VpnStoreGuard` 包装，避免部分更新后中途返回。

## 与装配层的边界
- `vpn-frame` 不关心 SQLite、HTTP API 或管理员账号初始化。
- `bucky-vpn-server` 负责提供 `VpnStoreFactory` 和 `VpnCmdServer`。
- 共享运行时只假设外部装配层提供稳定的：
  - 网络组真相
  - joined node 真相
  - network member 真相
  - 对等节点命令通道

## 风险点
- 若 store 查询语义不一致，可能出现成员存在但不可见、或可见但无法解析目标的状态。
- 若 `info_version` 未随授权变更递增，客户端可能长期持有过时拓扑。
- 若在线状态与 store 可见性判断顺序处理不当，可能造成短时间的可见节点抖动。
- relay 级 source-target 授权缺口仍然存在，需要后续依赖任务单独处理。
