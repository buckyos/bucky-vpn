# CYFS Gateway 配置领域说明

本文档固化了仓库内关于 `cyfs-gateway` 配置与 process-chain DSL 的领域真相，用于编写、审查或解释配置相关任务。

## 运行时真相

### 配置装载链
1. Load the main config file and its `includes`.
2. Merge in builtin control-plane defaults from `gateway_control_server.yaml`.
3. Treat that merged result as `user_config`.
4. If a saved patch exists, overlay it on `user_config` to get `effective_config`.
5. Runtime uses `effective_config`.

### include 与 merge 规则
- Root include shape:

```yaml
includes:
  - path: other.yaml
```

- include 支持本地文件、本地目录，以及 `http://` / `https://` 远程文件
- 本地 include 的相对路径，相对于当前 include 文件所在目录解释
- 远程 include 的相对路径，相对于当前 URL 的父目录解释
- merge 语义：
  - object + object：递归合并
  - array + array：去重追加
  - array + 单值：若不存在则追加
  - 其他类型：后者覆盖前者

### 路径归一化
- `path` 和所有 `*_path` 字段都会在最终解析前做统一归一化
- 相对配置路径一律相对于主配置文件目录解释，而不是 include 文件目录
- 类似 `a.json#fragment` 的值，只归一化路径部分，保留 fragment

### 顶层 map key 注入
- These sections use map keys as the real definition source:
  - `stacks`
  - `servers`
  - `timers`
  - `limiters`
  - `collections`
  - `global_process_chains`
- Runtime injects identifiers from map keys:
  - `stacks.<key>` -> `id = <key>`
  - `servers.<key>` -> `id = <key>`
  - `timers.<key>` -> `id = <key>`
  - `limiters.<key>` -> `id = <key>`
  - `collections.<key>` -> `name = <key>`
  - `global_process_chains.<key>` -> `id = <key>`
  - `blocks.<key>` -> `id = <key>`

### hook map
- 在 YAML 中，`hook_point`、`post_hook_point`、`on_new_tunnel_hook_point`、`blocks` 通常写成 map
- 解析阶段会把它们转成按 `priority` 排序的数组
- `global_process_chains` 也遵循同样的 map-to-array 规则

### 当前支持的类型
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
- `NdnServerConfig` 虽然在库代码中存在，但应用层当前没有注册 `ndn` 的 parser/factory，不能视为正式支持

## Process Chain 真相

### 承载位置
- Process chains live primarily in:
  - `stacks.<id>.hook_point`
  - `servers.<id>.hook_point`
  - `servers.<id>.post_hook_point`
  - `stacks.<rtcp>.on_new_tunnel_hook_point`
  - `global_process_chains`
  - `timers.<id>.process-chain` or `process_chain`

### 执行模型
- `block` 是最小执行单元
- `chain` 是包含多个 block 的有序执行体
- `hook_point` 是按优先级执行的 chain 列表
- 同一个 chain 内的 block 共享 chain 级变量
- 不同 chain 默认不共享 chain 局部变量
- 命中终止结果后，当前 chain 或 hook-point 不再按普通顺序继续向下执行

### DSL 规则
- 表达式链支持 `&&`、`||`、`!`、`;`、`()`
- statement 级语法包括：
  - `if / elif / else / end`
  - `for ... in ... then ... end`
  - `match-result ... end`
- 这些结构化语句属于语法结构，不是普通命令

### Canonical 命令名
- 需要统一使用当前 canonical 名称，例如：
  - `match-reg`
  - `rewrite-reg`
  - `call-server`
  - `match-include`
- 不要把旧文档中的下划线命名和当前短横线命名混用

## 推荐写法
- 先写运行时行为，再写推荐风格
- 先给最小可运行示例，再追加可选字段
- map key 才是定义源，除非目标类型明确要求，否则不要重复手写 `id` / `name`
- 示例里如果出现 `path` 或 `*_path`，必须明确说明它们相对于主配置文件目录解释
- 讲 process-chain 时，必须把以下三层分开：
  - 执行模型
  - 结构化语法
  - 具体命令
