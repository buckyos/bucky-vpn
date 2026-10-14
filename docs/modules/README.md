# 模块边界说明

本目录用于存放长期模块边界文档。

- 这些文档描述稳定的职责、输入、输出、依赖和边界
- 具体任务的 proposal/design/testing/acceptance packet 放在本地 `.harness/tasks/<version>/modules/<packet-module>/<task-seq>-<task-slug>/`
- standard 变更记录位于 `.harness/changes/`；索引只由 `harness/scripts/task-index.py` 管理
- fresh clone 从 `harness/templates/task-packet/` 和 `harness/templates/change-record.md` 初始化本地模板；`.harness/` 不随 Git 克隆
