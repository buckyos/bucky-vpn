---
module: repo-governance
version: v0.1
status: approved
approved_by: user-request
approved_at: 2026-04-22
---

# Repo Governance Acceptance 标准

## 范围
- In scope:
  - 顶层 AGENTS harness 入口
  - docs 骨架和模板
  - 稳定 harness 规则
  - cyfs-gateway 配置规范 overlay
  - canonical 治理验证入口
- Out of scope:
  - 产品功能行为
  - CI 自动化
  - 端到端打包验证

## 必需证据
- `proposal.md`
- `design.md`
- `testing.md`
- `testplan.yaml`
- `docs/`、`harness/` 和 `AGENTS.md` 下发生变化的治理文件
- `python3 ./harness/scripts/test-run.py repo-governance <level>` 的验证结果

## 通过条件
- Harness 目录和模板在 `AGENTS.md` 引用的路径上真实存在。
- 稳定规则已经覆盖 design、testing、implementation 准入、统一测试入口和 acceptance 行为。
- 稳定规则已经覆盖 proposal、design、testing、implementation 准入、统一测试入口、触发验证和 acceptance 行为。
- 代表性模块 packet 内部一致，且已经批准。
- implementation / bugfix 流程、贡献模式和模块分级都已经以仓库文件形式公开，而不是只存在于聊天上下文。
- `cyfs-gateway` overlay 保留了运行时真相：
  - map-key injection
  - merge semantics
  - path normalization
  - registered stack/server support
  - process-chain execution and syntax boundaries
- 治理类验证入口在结构错误时会返回非零退出码。

## 失败回流
- 工作流意图缺失或错误：退回 proposal
- 布局或规则设计错误：退回 design
- 验证面缺失或不正确：退回 testing
- 脚本行为或文件接线损坏：退回 implementation
