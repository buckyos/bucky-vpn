# 仓库工作流

本仓库在现有 Rust workspace 和 Flutter 前端的基础上，叠加了一层 retrofit 形式的 Harness Engineering 工作流。

## 任务读取顺序
1. 先读 `AGENTS.md`
2. 再读 `docs/versions/v0.1/modules/<module>/` 下当前模块 packet
3. 再读 `docs/modules/<module>.md` 了解长期边界
4. 再读 `docs/architecture/` 中的项目级约束
5. 再读 `harness/rules/` 中的稳定规则
6. 任务命中特定领域或 implementation/bugfix 流程时，再读 `harness/process_rules/` 和 `harness/checklists/`
7. 任务涉及高风险工件或需要判断协作自治级别时，再读 `harness/human-rules/`

## 阶段边界
- Proposal：负责沉淀目标、范围、非目标和约束。输出：`proposal.md`
- Design：负责定义实现结构、接口、依赖和落地顺序。输出：`design.md` 与可选 `design/`
- Testing：负责定义验证策略、稳定入口和通过条件。输出：`testing.md`、可选 `testing/` 与 `testplan.yaml`
- Implementation：只负责代码和测试代码实现。必须在 `proposal.md`、`design.md`、`testing.md` 都已存在、审批完成且已覆盖当前改动后才允许进入
- Acceptance：只负责审计证据链并输出独立评审报告，不在 acceptance 阶段直接修代码

## 验证面
- 面向人的文档：
  - `proposal.md`
  - `design.md`
  - `testing.md`
  - `acceptance.md`
  - `docs/versions/v0.1/reviews/` 下的独立评审报告
- 面向脚本的入口：
  - `testplan.yaml`
  - `python3 ./harness/scripts/test-run.py <module> unit`
  - `python3 ./harness/scripts/test-run.py <module> dv`
  - `python3 ./harness/scripts/test-run.py <module> integration`

## 治理 overlay
- 稳定规则：`harness/rules/`
- 任务流程 overlay：`harness/process_rules/`
- 领域 checklist：`harness/checklists/`
- 协作模式与模块分级：`harness/human-rules/`
- 高风险变更触发：`harness/custom-rules/trigger-based-validation-rules.md`

## 当前已初始化模块
- `vpn-frame`：共享 VPN 协议与运行时库
- `bucky-vpn`：客户端二进制
- `bucky-vpn-server`：服务端二进制
- `vpn_web`：Flutter Web 前端
