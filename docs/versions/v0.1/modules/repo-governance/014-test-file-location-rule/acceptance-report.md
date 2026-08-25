# 独立测试文件目录规则 Acceptance Report

## Findings
| ID | Severity | Kind | Evidence | Problem | Blocking |
|----|----------|------|----------|---------|----------|
| F-000 | none | implementation | `proposal.md` P-001；`harness/custom-rules/test-file-location-rule.md`；`harness/custom-rules/index.yaml`；`harness/tests/test_test_file_location_rule.py`；任务级测试产物 `.harness/test-results/test-runs/20260806T083945Z-repo-governance+014-test-file-location-rule-all.json` | 独立复核未发现需求缺失、规则矛盾、路由漏项或会掩盖本次治理行为的测试缺陷。 | no |

## Requirement Review
| Requirement or Boundary | Source | Implementation Evidence | Finding | Status |
|-------------------------|--------|-------------------------|---------|--------|
| 独立测试文件不得放在 `src` 或其子目录，必须放在对应 `src` 同级的 `tests`。 | `proposal.md` P-001 和用户原始要求 | `test-file-location-rule.md` 的“定义”“强制规则”和“阶段处理” | 规则明确覆盖根级与模块级源码树，并要求相同父目录下的 `tests`，没有将测试专用 package 或私有可见性作为绕过条件。 | pass |
| 内联测试不是独立测试文件，不要求从生产源码拆出。 | `proposal.md` Out of scope | `test-file-location-rule.md` 的“定义”和“允许边界” | Rust `#[cfg(test)] mod tests` 被明确列为内联边界，不会被目录规则误判为独立文件。 | pass |
| 当前任务触及历史违规独立测试文件时必须迁移，但本任务不批量迁移未触及文件。 | `proposal.md` Scope / Out of scope | `test-file-location-rule.md` 的“强制规则”和“与其他规则的关系” | 触及即迁移和不主动清理历史文件两个边界均被保留，没有扩大本次业务模块范围。 | pass |
| 规则应在 implementation、testing、acceptance 阶段通过测试 trigger 或 `src`/`tests` 路径被优先路由。 | `proposal.md` P-001；`design.md` File-Level Interfaces | `custom-rules/index.yaml` 的 `project-test-file-location` 条目；`test_test_file_location_rule.py` | 三个阶段、manual/auto-pipeline、三种 tier、根级与模块级路径均有声明；测试同时验证自定义规则出现在生成规则之前，并验证 Design 阶段不误激活。 | pass |
| 仓库治理入口和统一测试入口必须同步更新。 | `proposal.md` In scope；`trigger-based-validation-rules.md` | `docs/architecture/repository-workflow.md`；`harness/scripts/test-run.py` 的 `repo-governance` unit suite | 长期文档链接新规则，聚焦测试也进入既有统一入口，没有创建旁路测试命令。 | pass |

## Implementation Review
| Area | Evidence | Finding | Status |
|------|----------|---------|--------|
| requirement-and-behavior | `proposal.md` P-001 与 `test-file-location-rule.md` 全文逐项对照 | 规则保留用户要求的禁止位置、目标位置、内联边界和历史文件触及边界；未引入全仓扫描或业务迁移。 | pass |
| logic-and-control-flow | `custom-rules/index.yaml` 路由字段；`context.py` 实际输出；五个定向用例 | 索引使用既有 trigger/path 选择逻辑；根级、模块级、各执行阶段和 Design 排除分支均得到验证，未发现不可达或错误阶段分支。 | pass |
| boundary-and-input | `src/**`、`*/src/**`、`tests/**`、`*/tests/**`；额外人工探测 `deeply/nested/module/src/unit/example_tests.rs` | 根级、单模块和更深模块路径都能选中规则；正文把独立文件、测试辅助文件和内联测试边界分别说明。 | pass |
| state-and-data-integrity | 规则 Markdown、索引 YAML 和现有 `context.py` 消费方式 | not-applicable：交付只新增无状态治理元数据，不写产品状态、缓存或持久业务数据；索引与规则文件是一对一引用。 | pass |
| error-handling-and-recovery | `test_invalid_rule_reference_fails_index_validation` | 缺失规则文件的索引引用会使 `context.py --validate-index` 非零退出并报告 missing，不会静默忽略失效规则。 | pass |
| resource-lifetime-and-cleanup | `test_invalid_rule_reference_fails_index_validation` 的 `TemporaryDirectory`；规则/索引不创建运行时资源 | not-applicable：生产交付不持有文件句柄、锁、线程或网络资源；测试夹具由上下文管理器清理。 | pass |
| concurrency-and-ordering | `design.md` Implementation Order；`pipeline/plan.md` I-001 → I-002 → T-1 | not-applicable：规则读取是无状态配置消费，没有并发可变状态；实现和验证按显式依赖顺序完成。 | pass |
| interface-and-compatibility | `project-test-file-location` 索引条目；`vpn-web-no-new-tests-rule.md`；规则“与其他规则的关系” | 只新增既有 schema 支持的条目，不修改路由器接口；`vpn_web` 是否新增测试仍由原规则决定，本规则只约束已存在测试文件的位置。 | pass |
| security-and-capacity | 新规则文本、固定索引条目和 `context.py --validate-index` | not-applicable：没有用户输入执行、秘密、反序列化或增长型存储；固定的四个路径模式不会引入新的运行时容量面。 | pass |
| test-adequacy | `testplan.yaml`；五个定向用例；成功任务产物 | 测试覆盖正常路由、根级/模块级边界、Design 负向排除、缺失规则错误和优先级兼容性；生命周期与跨产品模块运行时行为有具体不适用理由。测试不能机械扫描未来提交，但该扫描明确属于批准的非目标。 | pass |

## Document Consistency
| Document | Source | Implementation Consistency | Finding | Status |
|----------|--------|----------------------------|---------|--------|
| design | `design.md` | 新规则文件先创建，随后登记索引；条目字段、阶段、模式、tier 和路径模式与设计一致。 | 实现遵循规则单一权威来源、既有路由器消费和不修改 `context.py` 的设计边界。 | pass |
| testing | `testplan.yaml` | 任务产物只执行声明的 `test-file-location-rule-routing` unit step，change_id 与 evidence inputs 完整绑定。 | 自动测试的覆盖状态与实际五个用例一致；DV/integration 因无产品模块或跨模块运行时合同而禁用，理由具体。 | pass |

## Result Summary
- Overall result: accepted
- Outcome: 已新增并激活独立测试文件目录规则；后续相关任务会被要求把独立测试文件放入与 `src` 同级的 `tests`。
- Blocking issues: none；独立复核未发现需求、实现、设计一致性或测试一致性阻塞项。
- Next action: 完成自动流水线的验收回执、完整性检查和任务索引移除。

## Object and Scope
- Task manifest: task.yaml
- Reviewed change: `CHG-enforce-test-file-location`
- In scope: 已批准 Proposal/Design、新规则、索引路由、治理文档、统一测试入口、聚焦测试、testplan 和成功任务产物。
- Out of scope: 未触及历史测试文件的批量迁移、全仓路径扫描器、产品代码和当前工作树中的无关改动。
- Review independence: 当前环境未启用独立 reviewer agent；验收按 fresh-source 顺序重新检查原始需求、实现、消费者、测试代码和运行产物，未沿用实现阶段结论作为正确性证据。

## Conclusion
- Accepted / rejected / needs changes: accepted
- Reason: 规则正文完整表达用户批准的目录约束，索引在所有目标阶段和路径边界中可路由且优先于生成规则，治理入口与统一测试入口均已更新，任务级验证覆盖了适用的正常、边界、负向、错误和兼容性风险。
- Supporting task-relevant test evidence: `.harness/test-results/test-runs/20260806T083945Z-repo-governance+014-test-file-location-rule-all.json`，exit code 0，绑定 `CHG-enforce-test-file-location`。
- Residual risk: 本规则依赖 Agent 按上下文执行，不包含扫描 changed-path manifest 的机械拦截；这是已批准 Proposal 的明确非目标，未来如需强制扫描应单独设计 Harness 工具变更。
