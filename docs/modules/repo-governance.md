# repo-governance

## 角色
负责仓库级 harness 工作流、规则、模板和验证入口。

## 输入
- `AGENTS.md`
- `docs/architecture/`
- `harness/rules/`
- `harness/process_rules/`
- `harness/human-rules/`
- 贡献者与智能体复用的稳定验证命令

## 输出
- 可持续复用的仓库工作流
- 阶段模板与评审模板
- 协作模式与模块分级治理
- 面向脚本的 harness 验证入口
- 类似 `cyfs-gateway` 配置规范这类领域 overlay

## 依赖
- Python 3
- 仓库目录约定

## 边界
- 只负责工作流脚手架和治理文档
- 不应擅自改写产品模块的业务行为或 acceptance 标准
- 负责公开哪些模块允许多大自治度、哪些变更必须触发额外验证
