---
module: repo-governance
version: v0.1
status: approved
approved_by: user-request
approved_at: 2026-04-22
---

# Repo Governance Proposal

## 背景与目标
仓库原本已经有项目级构建命令和代码风格说明，但缺少稳定的 Harness Engineering 骨架。本 proposal 的目标是在不改写现有产品布局的前提下，补齐可复用的工作流规则、模板和验证入口。

## 范围
### In scope
- 初始化 `docs/`、`harness/` 和评审模板结构
- 补齐 proposal、implementation flow、trigger-based validation 等稳定规则缺口
- 增加 contribution mode 与 module tier matrix 这类人机协作治理工件
- 定义阶段边界与 implementation 准入规则
- 增加一个代表性的仓库治理模块 packet
- 增加一个仓库内的 `cyfs-gateway` 配置/DSL 领域 overlay
- 增加一个稳定的、机器可读的 harness 验证入口

### Out of scope
- 产品功能改动
- CI 系统改动
- 打包流程改动
- 一次性把所有产品模块都补成完整 packet

### Boundary with neighboring modules
- 产品模块继续保持现有代码归属和构建命令
- `repo-governance` 只负责工作流脚手架和领域任务 overlay

## 约束
- 允许使用的库/组件：Python 3 标准库、现有仓库工具、当前 Rust/Flutter 命令
- 禁止采用的方案：替换现有仓库布局、引入要求新增依赖的工作流工具
- 系统约束：改动必须是增量式的，AGENTS 必须兼容现有命令，验证必须非交互

## 高层结果
- 后续智能体有稳定的文档与规则读取路径
- 版本化模块工作在没有 proposal/design/testing 工件前不能直接进入 implementation
- 高风险改动有公共触发规则，不能再只靠默认验证路径兜底
- 人机协作模式与模块自治边界对仓库贡献者公开可见
- 配置规范类任务不再依赖聊天上下文，而是依赖仓库内稳定规则

## 风险
- 在真实 packet 还不多时，可能过早把流程写得过细
- 规则层次变多后，如果入口文档不够清楚，会增加智能体加载成本
- 如果模块命令后续变化，harness 文档与真实验证命令可能漂移
