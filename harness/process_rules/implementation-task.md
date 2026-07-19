# Implementation / Bugfix 任务流程

当任务进入 implementation 或 bugfix 阶段时，使用本流程，而不是直接从聊天上下文开始改代码。

## 读取顺序
1. 读取 `AGENTS.md` 与 `harness/rules/task-entry-gate-rules.md`，选择当前 `task.yaml`
2. 运行 `harness/scripts/harness-check.py --task <packet>/task.yaml --profile pre-edit`
3. 只读取 context router 输出的 proposal、active design source、模块边界、架构约束和匹配规则
4. 按 `harness/rules/implementation-rules.md` 执行实现

## 开始实现前必须确认
- manual flow 的 `proposal.md`、`design.md` 都存在且 `status: approved`；auto-pipeline 使用 launch-confirmed proposal 与已验证的 pipeline plan mappings
- 当前变更能映射到明确的 proposal、active design source、`change_id` 与 `Scope Paths`
- 如果 approved 文档没有覆盖当前变更，先退回对应上游阶段补文档
- 不能用 `docs/modules/<module>.md`、历史记忆或聊天描述替代当前变更的直接覆盖

## 实施规则
- implementation 只改 production code 与必需的非测试 runtime/build resources；测试设计和测试代码属于后置 testing 阶段
- bugfix 默认也遵循同一准入规则，除非仓库未来增加明确例外
- 改动必须最小化，优先落在当前模块边界内
- 命中触发规则时，完成其 implementation 所需检查；测试覆盖在 implementation 完成后由 testing 阶段补齐

## 验证规则
- 不要把“顺手跑一下”当成默认动作
- 只有在以下情况之一成立时，implementation 才应主动执行验证：
  - 用户明确要求
  - 调试需要新的失败证据
  - 当前模块 `testing.md` 或仓库规则把该验证声明为本任务必需入口
- 如果验证未执行，交付说明必须明确指出未执行项和原因

## 回流规则
- 缺 proposal 或范围不清：退回 proposal
- 缺 design 或设计未覆盖当前改动：退回 design
- 实现完成后缺 testing 或验证面未覆盖当前改动：进入 testing
- 发现模块边界本身失真：先同步 `docs/modules/<module>.md` 的设计任务，再继续实现
