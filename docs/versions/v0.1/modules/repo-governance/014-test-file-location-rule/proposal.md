---
task_manifest: task.yaml
status: approved
---

# 独立测试文件目录约束提案

Risk profile: ./risk-profile.yaml

## Workflow Tier Judgment
- Proposed tier: high-risk
- Final tier: high-risk
- Tier rationale / triggered boundaries: 本变更新增一条全仓库生效的项目自定义 Harness 规则，并改变后续实现、测试和验收任务对测试文件布局的准入判断；命中 `harness` process trigger 和项目 `repo-governance` 自定义验证规则，因此建议按 high-risk 流程执行。
- Proposal and tier confirmation: 用户于 2026-08-06 回复“确认”，批准本提案及显示的 high-risk 级别；当时没有未决问题。

## Background and Goal
仓库中各模块通常以 `src` 保存生产源码。独立测试文件如果也放入 `src` 或其子目录，会混淆生产源码与测试代码的目录边界，并容易让后续 Agent 继续复制这一布局。

目标是新增一条项目自定义 Harness 规则：独立测试文件禁止位于任何 `src` 目录及其子目录，必须位于与对应 `src` 同级的 `tests` 目录中。

## Scope
### In scope
- 新增 `harness/custom-rules/test-file-location-rule.md`，明确“独立测试文件”的判断标准、强制目录位置以及实现/测试/验收阶段的处理方式。
- 将新规则登记到 `harness/custom-rules/index.yaml`，在 implementation、testing、acceptance 阶段按测试相关 trigger 和 `src`/`tests` 路径路由。
- 明确本规则适用于任务新增、移动或修改的独立测试文件；任务触及已位于 `src` 下的独立测试文件时，必须在任务完成前迁移到同级 `tests` 目录。
- 更新 `docs/architecture/repository-workflow.md`，让仓库工作流入口能发现该项目级目录约束。
- 增加聚焦的 `repo-governance` 验证并登记到 `harness/scripts/test-run.py`，验证自定义规则被索引且能针对代表性的 `src` 测试文件路径正确路由。

### Out of scope
- 本任务不批量迁移仓库中所有既有独立测试文件；未被本任务修改的历史文件仅作为后续任务适用规则时的待处理项。
- 不改变生产源码文件内部已有的内联测试块；内联测试代码不是“独立测试文件”。
- 不新增全仓文件扫描器，也不改动产品、运行时、构建或测试框架行为。
- 不重写现有 Harness 生成规则，也不清理当前工作树中的无关修改。

### Boundary with neighboring modules
该约束由 `repo-governance` 的项目自定义规则、索引和治理验证入口负责。各业务模块只在后续任务新增、移动或修改独立测试文件时遵守目录布局，不在本任务中修改业务模块代码。

## Requirement Review
该要求合理，能够建立清晰、可复制的测试目录边界。需要显式保留的语言边界是：以 Rust 为例，将测试放入 crate 根目录的 `tests` 通常意味着按集成测试边界编译，不能依赖生产模块的私有实现；后续测试如果必须验证私有细节，应优先改为通过公开行为验证或调整可测试边界，而不是把独立测试文件重新放回 `src`。

本任务选择“索引路由 + 明确 review/acceptance 规则 + 聚焦治理测试”的方式固化约束。这样能验证规则可被 Harness 加载，并保持本次范围为规则维护；若未来需要对 changed-path manifest 做强制扫描，应作为独立的 Harness 工具变更提出。

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-enforce-test-file-location | 独立测试文件不得位于 `src` 或其任何子目录，必须放入与对应 `src` 同级的 `tests`；任务触及历史违规文件时也必须迁移。 | 仅新增项目自定义规则、索引、治理说明与规则路由验证；不修改业务模块测试文件。 | Rust 独立测试将遵循集成测试可见性边界；聚焦验证证明规则可路由，但不提供全仓扫描式机械拦截。 | `context.py --validate-index` 通过；代表性 `src/*_tests.rs` 路径能路由到新规则；`python3 ./harness/scripts/test-run.py repo-governance unit` 中新增的聚焦用例通过；治理文档引用新规则。 | 不迁移未触及的历史文件，不改变内联测试，不新增通用扫描器。 |

## Success Criteria
- Concrete user-visible or system-visible result: 后续 Agent 在创建、移动或修改独立测试文件时，会收到清晰且最高优先级的项目规则，要求使用与 `src` 同级的 `tests` 目录。
- Required evidence: 自定义规则索引有效；代表性根级和模块级 `src` 测试路径均能路由到该规则；聚焦 `repo-governance` 测试经统一入口通过；治理文档与规则内容一致。
- Explicit non-goals: 本任务不修改产品行为、不批量迁移既有测试文件、不改变内联测试政策、不实现全仓路径扫描器。

## Risks
- 如果路径模式只覆盖 `src/**` 或只覆盖 `<module>/src/**`，另一类目录形态可能无法触发规则；验证必须同时覆盖根级与模块级示例。
- 仅有文字规则而没有正确索引会导致规则无法进入 Agent 上下文；必须同时验证索引完整性和路由结果。
- 将独立 Rust 测试迁移到 `tests` 会失去私有项可见性；本规则接受这一约束，并要求后续任务通过公开行为或明确的可测试边界解决，而不是保留 `src` 下独立测试文件。
- 当前工作树已有大量无关修改，执行阶段必须保持这些内容不变，并将本任务变更限制在 `task.yaml` 声明的治理路径。
