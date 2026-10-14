# 仓库工作流

本仓库在 Rust workspace 与 Flutter 前端上使用 Harness Engineering。入口是 AGENTS.md；规则索引和 context.py 提供当前阶段推荐上下文。Scope Paths 与 stage-scope 承担可追溯性和完成证据，文件访问由外部权限与用户范围决定。

所有任务材料位于 gitignored .harness：packet 使用 .harness/tasks/<version>/modules/<packet-module>/<task-seq>-<task-slug>/，standard 变更记录使用 .harness/changes/<change>.md。docs/modules 和 docs/architecture 保存长期知识。fresh clone 不包含本地任务、索引或 receipt；持久工具模板位于 harness/templates，可重新初始化本地材料。

## 任务流与责任

普通任务先只读发现边界，创建 common proposal packet，以 pending tier 路由 proposal，展示需求/范围/tier 并获得用户确认。直接维护 Harness 规则政策的例外与 GitHub issue 提交额外确认由 task-entry-gate-rules 和 github-issue-creation-rules 分别拥有。

standard 使用一个变更记录、比例适当的验证和独立发现缺陷的 completion-report；high-risk 依次执行 proposal、design、implementation、testing、acceptance。每个 high-risk 阶段使用没有父上下文的新子代理；父代理负责确认、注册、合法转换和收口。实施后的 testing 负责完整测试设计与实现，acceptance 独立判断交付缺陷。

proposal/tier 确认授权必要下游执行，不重复询问阶段许可。task-transition.py 拥有 high-risk completion check、内容绑定 receipt 和 stage 转换；不手改 stage 或 receipt。失败返回负责阶段，修改需求或验收边界时返回 proposal 重新确认。standard 收口由 task-index.py remove 验证 baseline、实际 changed paths、完成记录和独立报告。

## 验证与证据

统一入口是 python ./harness/scripts/test-run.py <module-or-module/task-name> <unit|dv|integration|all>。任务运行从当前 testplan 注册并写 .harness/test-results 下的机器证据；module suite 保留项目实际命令。治理验证不能因 harness/docs 改动运行无关 Rust workspace 或 Flutter 测试。check-all 是 scaffold audit，不回放历史任务验收。

receipt 依赖当前批准内容、Scope Paths 和有效运行输入；新失败或输入改变使旧通过不能复用。schema/scope/run artifact 是完成证据，不能替代独立缺陷发现。质量检查只在用户明确要求或匹配 custom 策略时执行。

## 项目政策与历史迁移

harness/custom-rules 保持项目优先级与原字节；工具对已知旧 modes/trivial 元数据作限定兼容，新 generated 索引仍严格，旧 trivial 不是新任务选项。专用领域流程、清单、human-rules 与实际产品注册保留其责任。

历史任务迁至 .harness 并保留原件、逐文件 hash 和状态出处；task-index.py init/add 按真实未完成状态重建索引。缺 manifest 历史是档案，不补造审批。旧 receipt/run/launch 只说明当时事实，不重新签成当前通过，不代关闭未完成产品任务。

039 的已批准前置同步只覆盖其 draft testing.md/testplan.yaml 治理验证面，绑定 canonical packet、批准 proposal hash 与限定阶段；完整案例、代码、运行证据仍由实施后的 testing 提供。

长期责任模块：vpn-frame、bucky-vpn、bucky-vpn-server、vpn_web 与 repo-governance。具体任务事实仅存在本地 packet，边界见 docs/modules。
