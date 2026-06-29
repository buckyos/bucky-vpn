# Implementation / Bugfix 任务流程

当任务进入 implementation 或 bugfix 阶段时，使用本流程，而不是直接从聊天上下文开始改代码。

## 读取顺序
1. 当前模块 `proposal.md`
2. 当前模块 `design.md` 与必要的 `design/`
3. 当前模块 `testing.md` 与 `testplan.yaml`
4. `docs/modules/<module>.md`
5. `docs/architecture/` 中相关项目级约束
6. `harness/rules/implementation-admission-rules.md`
7. 如果命中高风险触发条件，再读 `harness/rules/trigger-based-validation-rules.md`
8. 如果任务自治级别或评审要求不清楚，再读 `harness/human-rules/`

## 开始实现前必须确认
- `proposal.md`、`design.md`、`testing.md` 都存在且 `status: approved`
- 当前变更能映射到明确的 proposal、design、testing 条目
- 如果 approved 文档没有覆盖当前变更，先退回对应上游阶段补文档
- 不能用 `docs/modules/<module>.md`、历史记忆或聊天描述替代当前变更的直接覆盖

## 实施规则
- implementation 只改代码和测试代码
- bugfix 默认也遵循同一准入规则，除非仓库未来增加明确例外
- 改动必须最小化，优先落在当前模块边界内
- 命中触发规则时，先补齐 testing 覆盖和评审关注点，再继续编码

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
- 缺 testing 或验证面未覆盖当前改动：退回 testing
- 发现模块边界本身失真：先同步 `docs/modules/<module>.md` 的设计任务，再继续实现
