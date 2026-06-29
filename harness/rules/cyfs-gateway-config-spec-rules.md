# CYFS Gateway 配置规范规则

## 目标
- 为编写、评审、讲解 `cyfs-gateway` 配置和 process-chain DSL 的任务提供稳定的仓库内规则

## 必需读取顺序
1. `docs/architecture/cyfs-gateway-config-domain.md`
2. `harness/process_rules/cyfs-gateway-config-task.md`
3. `harness/checklists/cyfs-gateway-config-review-checklist.md` for review or acceptance tasks

## 运行时真相
- `user_config` 已经包含了控制面的内置默认值
- `effective_config` 是 `user_config` 再叠加 patch 后的最终运行时配置
- include 的相对路径相对于当前 include 文件或 URL 解释
- `path` 与 `*_path` 的相对路径相对于主配置文件目录解释
- `stacks`、`servers`、`timers`、`limiters`、`collections`、`global_process_chains` 都是 map 定义源，key 会变成运行时标识

## 支持类型规则
- 已注册的 stack 协议：
  - `tcp`
  - `udp`
  - `tls`
  - `quic`
  - `rtcp`
  - `tun`
- 已注册的 server 类型：
  - `http`
  - `socks`
  - `dns`
  - `dir`
  - `control_server`
  - `local_dns`
  - `sn`
  - `acme_response`
- 不要把 `ndn` 说成应用层已支持。虽然库中存在 `NdnServerConfig`，但应用当前并未注册

## Process-chain 规则
- 输出时必须把三类内容明确分开：
  - 执行模型
  - `if/elif/else/end`、`for ... end`、`match-result ... end` 这类结构化语法
  - 具体命令及其 canonical 名称
- 命令名应使用 canonical 写法，例如 `match-reg`、`rewrite-reg`、`match-include`
- 不要混用旧式下划线命名

## 输出规则
- 先写运行时行为，再写推荐风格
- 示例先给最小可运行配置，再追加可选字段
- 如果任务同时涉及配置结构和 process-chain，两个都要覆盖，不能只讲 YAML 形状
