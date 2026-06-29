# CYFS Gateway 配置任务流程

当任务涉及 `cyfs_gateway.yaml`、拆分配置文件、配置文档或 process-chain DSL 示例时，使用本 overlay。

## 执行顺序
1. 先明确运行时真相：
   - 装载链
   - merge 语义
   - 路径归一化
   - map key 注入
   - 支持的 stack/server 类型
2. 如果任务包含 process chain，再把以下三层分开说明：
   - 执行模型
   - 结构化语法
   - 具体命令语法
3. 只有在运行时真相讲清楚之后，才能补推荐写法和示例

## 编写规则
- 必须区分 `user_config` 与 `effective_config`
- 示例中出现 `path` 或 `*_path` 时，必须声明其相对于主配置文件目录解释
- `hook_point`、`post_hook_point`、`on_new_tunnel_hook_point`、`blocks`、`global_process_chains` 要按“YAML 中写 map，运行时转数组”来描述
- 讲逻辑控制时，必须同时覆盖表达式链运算符和 statement 级语法
- 如果任务要求“命令精确写法”，只能使用 canonical 命令名

## Review 触发条件
- 改动了支持类型列表的说法
- 新增或修改了 process-chain DSL 示例
- 解释了 include、merge 或路径解析行为
- 提到了 `ndn`

## Review 必查项
- 协议与 server 类型列表必须与当前注册集完全一致
- 如果提到 `ndn`，必须明确标注为应用层未注册
- 示例不得混淆 include 相对路径与配置字段路径归一化
- 结构化语法不得被描述成普通命令
