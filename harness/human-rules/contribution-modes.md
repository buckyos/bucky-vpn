# 贡献模式

## 目标
- 公开说明本仓库在人和智能体协作时允许的工作模式，以及不同模式下对证据和审批的要求

## 模式定义
| 模式 | 适用场景 | 人的职责 | 智能体职责 | 最低证据要求 |
|------|----------|----------|------------|--------------|
| pure-human | 人工维护或紧急人工修复 | 自行承担范围、设计和验证判断 | 不参与执行 | 至少回写仓库文档，避免知识只留在线下 |
| human-agent loop | 人先定范围/审批，再让智能体执行 | 提供目标、确认 proposal/design/testing 或明确授权例外 | 按仓库规则读取、实现、报告未完成证据 | 当前模块 packet、相关规则、必要的验证结果或未执行原因 |
| agent-human loop | 先由智能体起草，再由人审批/收敛 | 审批或驳回 proposal/design/testing，决定是否进入实现 | 起草文档、实现或评审报告，但不能跳过审批门禁 | 起草后的阶段文档、审批状态、回流建议 |

## 本仓库默认用法
- 新功能、范围变更、协议合同调整：优先 `agent-human loop` 进入 proposal/design/testing，然后再切换到 `human-agent loop` 做 implementation
- 已批准范围内的实现、重构和普通 bugfix：默认 `human-agent loop`
- acceptance：优先保留独立审计角色，不把 acceptance 和 implementation 混成一个任务

## 通用约束
- 哪怕处于 `human-agent loop`，implementation 也不能跳过 `proposal.md`、`design.md`、`testing.md` 的批准门禁
- 哪怕处于 `agent-human loop`，proposal/design/testing 没有批准前也不能把草稿当成实现授权
- 如果当前模式无法覆盖所需风险控制，应升级到更严格的模式，而不是降低证据要求
