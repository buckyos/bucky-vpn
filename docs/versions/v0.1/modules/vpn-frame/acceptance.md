---
module: vpn-frame
version: v0.1
status: approved
approved_by: user-request
approved_at: 2026-04-16
---

# vpn-frame Acceptance 标准

## 范围
- In scope:
  - 共享协议与运行时库边界
  - crate 级与 workspace 级验证入口
- Out of scope:
  - 客户端二进制装配细节
  - 服务端二进制装配细节
  - UI 行为

## 必需证据
- `proposal.md`
- `design.md`
- `testing.md`
- `testplan.yaml`
- 相关 `vpn-frame` 代码改动
- 相关 Rust 测试或构建结果

## 通过条件
- 协议、client 运行时和 server 运行时之间的边界保持清晰
- 验证入口覆盖了 crate 内风险和 workspace 消费风险
- Acceptance 结论能区分共享库设计问题和下游集成问题

## 失败回流
- 模块范围或归属错误：退回 proposal
- 结构设计或边界映射错误：退回 design
- 验证面缺失：退回 testing
- 实现失败或兼容性回归：退回 implementation
