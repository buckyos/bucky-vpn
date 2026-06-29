---
module: bucky-vpn
version: v0.1
status: approved
approved_by: user-request
approved_at: 2026-04-16
---

# bucky-vpn Acceptance 标准

## 范围
- In scope:
  - 客户端二进制装配与本地 API 归属
  - CLI、设置和 daemon 启动行为
  - 客户端验证入口
- Out of scope:
  - 属于 `vpn-frame` 的共享运行时内部实现
  - 服务端行为
  - Web UI

## 必需证据
- `proposal.md`
- `design.md`
- `testing.md`
- `testplan.yaml`
- 相关 `vpn-client` 代码改动
- 相关 Rust 测试或构建结果

## 通过条件
- 客户端装配边界在 CLI、设置、API 和 daemon glue 之间保持清晰
- 验证入口覆盖 crate 内风险和 workspace 兼容性风险
- Acceptance 结论能区分客户端装配问题和共享运行时问题

## 失败回流
- 范围或归属错误：退回 proposal
- 结构设计或依赖映射错误：退回 design
- 验证面缺失：退回 testing
- 客户端行为失败或兼容性回归：退回 implementation
