---
module: bucky-vpn-server
version: v0.1
status: approved
approved_by: user-request
approved_at: 2026-04-16
---

# bucky-vpn-server Acceptance 标准

## 范围
- In scope:
  - 服务端二进制装配与存储/API 边界
  - 启动、配置、identity 与管理员初始化行为
  - 服务端验证入口
- Out of scope:
  - 属于 `vpn-frame` 的共享运行时内部实现
  - 客户端行为
  - Web UI

## 必需证据
- `proposal.md`
- `design.md`
- `testing.md`
- `testplan.yaml`
- 相关 `vpn-server` 代码改动
- 相关 Rust 测试或构建结果

## 通过条件
- 服务端专属职责在启动、持久化和 API 层之间保持清晰
- 验证入口覆盖 crate 风险和 workspace 兼容性风险
- Acceptance 结论能区分服务端装配问题和共享运行时问题

## 失败回流
- 范围或归属错误：退回 proposal
- 结构设计或依赖映射错误：退回 design
- 验证面缺失：退回 testing
- 服务端行为失败或兼容性回归：退回 implementation
