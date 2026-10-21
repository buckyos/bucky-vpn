# repo-governance 模块边界

repo-governance 负责仓库 Harness 工具、任务模板、规则路由、生命周期证据、统一测试入口与本地任务迁移。它消费项目策略，不能覆盖 harness/custom-rules 的优先级，也不负责 VPN Rust/Flutter 产品行为。

工具位于 harness/scripts；可共享和重新初始化的模板位于 harness/templates。所有实例任务、变更记录、索引、receipt、运行结果与缓存位于 gitignored .harness。任务 packet 使用 .harness/tasks/<version>/modules/<packet-module>/<task-seq>-<task-slug>/；standard 变更记录使用 .harness/changes/<change>.md。fresh clone 从持久模板初始化，不依赖本地历史工件。

task-index.py 独占未完成索引写入，task-transition.py 独占 high-risk stage/receipt 转换；test-run.py 写真实运行结果。治理 module suite 与产品实际 MODULE_SUITES 是不同责任，治理升级不替换产品命令或运行无关产品测试。项目专用流程和脚本保留其领域责任。

task_manifest.py 统一只读校验内的解析、字节和 hash，以及实际引用的递归设计来源；该缓存不跨越命令和写入。harness-check.py 复用完整输入绑定未变的 schema/risk/scope 结果；task-transition.py 的 refresh-documentation 仅更新解释性文档及审查绑定，保留有效运行并要求独立 acceptance 重审。行为或合同修正使用正常 return。

test-run.py 用 --task 精确绑定版本和 packet，module suite 不依赖历史 testplan。运行结果按 scope 和 packet 路径分区，保留唯一文件、前后输入快照、超时/取消及进程树清理结果；lifecycle-check.py 判断最新结果后才判定成功。产品命令和 vpn_web 工作目录保持兼容；custom index 的旧 modes/trivial 仅在 canonical custom-rules 索引中兼容，generated 元数据仍严格。

历史迁移保留原始材料和 hash 映射，只规范必要路径和活跃任务绑定。档案结论不代表当前工具通过；不得重签旧 receipt、推断关闭未完成任务或代验收产品任务。039 的两文件前置同步授权是当前用户确认的单任务例外，不构成后续工作流规则。

规范与阶段责任从 AGENTS.md 及 harness/rules/index.yaml 路由；全局协作边界见 docs/architecture/repository-workflow.md。
