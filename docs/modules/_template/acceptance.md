---
module: example-module
version: v0.1
status: draft
approved_by:
approved_at:
---

# [模块名] Acceptance 标准

## 范围
- In scope：
- Out of scope：

## 必需证据
- `proposal.md`
- `design.md`
- `testing.md`
- `testplan.yaml`
- implementation
- test code
- test results

## 通过条件
- 已批准 proposal 的意图没有漂移
- 设计、实现和测试三者保持一致
- 必需的验证入口已运行，或明确说明为何暂未运行
- 阻塞问题能被准确路由回对应阶段

## 失败回流
- Proposal 问题：退回 proposal
- Design 问题：退回 design
- Testing 问题：退回 testing
- Implementation 问题：退回 implementation
