# CYFS Gateway 配置评审清单

- 是否清楚区分了 `user_config` 和 `effective_config`
- 是否把 include merge 说明为递归 merge 与去重追加，而不是简单覆盖
- 是否明确说明相对 `path` / `*_path` 按主配置文件目录解释
- 是否把 `stacks`、`servers`、`timers`、`limiters`、`collections`、`global_process_chains` 视为 map 定义源，且 key 会注入为运行时标识
- 是否把已注册 stack 协议准确列为 `tcp`、`udp`、`tls`、`quic`、`rtcp`、`tun`
- 是否把已注册 server 类型准确列为 `http`、`socks`、`dns`、`dir`、`control_server`、`local_dns`、`sn`、`acme_response`
- 如果提到 `ndn`，是否明确说明应用层当前未注册
- 如果涉及 process chain，是否把表达式链、结构化语句和具体命令分开讲
- 如果展示命令名，是否统一使用 `match-reg`、`rewrite-reg`、`match-include` 等 canonical 写法
