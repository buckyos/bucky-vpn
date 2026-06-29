---
module: repo-governance
version: v0.1
status: approved
approved_by: user-request
approved_at: 2026-04-22
---

# Repo Governance Design

> 本文件只描述工作流设计。执行方式和通过条件放在 `testing.md`。

## 设计范围
### Goals
- 在现有仓库外层补一套最小但完整的 harness 骨架
- 保持顶层说明足够短，确保每次任务都能快速读取
- 为治理类工件提供一个稳定验证入口
- 保留现有 Rust、Flutter 与打包工作流

### Non-goals
- Auto-pipeline 编排
- CI 集成
- 第一天就为每个模块生成完整 packet

## 总体方案
使用一套三层 retrofit 布局：
1. `AGENTS.md` 仍然是每次任务都要读取的顶层入口，负责把人和智能体引向正确规则
2. `docs/` 承载长期架构文档、模块边界、版本化 packet 和评审模板
3. `harness/` 承载稳定规则、任务 overlay、checklist、human-rules 和脚本入口

代表性的 `repo-governance` packet 用来证明这套模式可行，而不要求先迁移产品模块。

## 模块拆分
| 子模块 | 类型 | 职责 | 输入 | 输出 | 依赖 | 是否单独文档 |
|--------|------|------|------|------|------|--------------|
| AGENTS 入口 | shared | 指向正确的文档与规则 | 仓库约定 | 稳定任务读取顺序 | `AGENTS.md` | no |
| docs 骨架 | shared | 承载架构文档、模块边界、版本化 packet 与评审模板 | 工作流模型 | 版本化仓库真相 | `docs/` | no |
| 稳定规则 | shared | 编码阶段不变量与准入门禁 | 工作流模型 | 可重复的 agent 行为 | `harness/rules/` | no |
| 领域 overlay | shared | 编码 cyfs-gateway 配置与 process-chain 约束 | 配置规范知识 | 任务级规则 | `harness/process_rules/`, `harness/checklists/` | no |
| 协作治理 | shared | 公开贡献模式、模块分级和高风险触发规则 | 模块边界与风险模型 | 人机协作约束 | `harness/human-rules/`, `harness/rules/trigger-based-validation-rules.md` | no |
| 测试入口 | shared | 暴露 canonical 机器可读验证入口 | 模块与级别选择 | 非交互验证 | `harness/scripts/test-run.py` | no |

## 实现顺序
| 阶段 | 目标 | 前置条件 | 产出 | 依赖 | 可否并行 |
|------|------|----------|------|------|----------|
| 1 | 更新顶层入口 | 已理解现有 `AGENTS.md` | 带 harness 语义的 AGENTS | none | no |
| 2 | 增加 docs 与模板骨架 | 阶段 1 完成 | docs 架构/模块/模板 | 1 | yes |
| 3 | 增加稳定规则、领域 overlay 与协作治理 | 阶段 1 完成 | `harness/rules/`、`harness/process_rules/`、`harness/human-rules/`、checklist | 1 | yes |
| 4 | 增加验证入口与代表性 packet | 阶段 2、3 完成 | 脚本和 `repo-governance` packet | 2,3 | no |

## 关键决策
- `AGENTS.md` 采用增量式扩展而不是整体重写，因为仓库里已有有效命令说明
- 选择 `repo-governance` 作为第一个真实模块 packet，因为当前任务本身是工作流初始化，而不是产品功能实现
- 把 `cyfs-gateway` 配置知识建模成领域 overlay，而不是仓库里的产品模块
- 把实现流程、贡献模式和模块分级显式固化到 `harness/process_rules/` 与 `harness/human-rules/`，避免自治边界只存在于口头约定
- 选择小型 Python 脚本作为稳定测试入口，因为 Python 3 通用且已被 harness 方案推荐

## 数据与状态
- 工作流状态存放在版本化 Markdown/YAML 文件里
- 审批状态通过 module packet 的 front matter 表达：
  - `status`
  - `approved_by`
  - `approved_at`
- 验证状态通过 `harness/scripts/test-run.py` 的退出码表达

## 接口与依赖
### 对外接口摘要
- `docs/versions/v0.1/modules/<module>/` 下的模块 packet 路径
- `docs/modules/` 下的长期模块边界文档
- `harness/rules/` 下的稳定规则
- `harness/scripts/test-run.py` 提供的 canonical 验证命令

### 对外 HTTP 接口细节
- 本模块没有 HTTP 接口

### 对外代码接口细节
- `python3 ./harness/scripts/test-run.py <module> <unit|dv|integration>`

### 依赖接口与外部约束
- Rust 与 Flutter 命令仍然是产品验证的事实来源
- 治理类验证不能要求网络访问，也不能要求新增依赖

## 实现布局
```text
repo-root
├── AGENTS.md
├── docs/
│   ├── architecture/
│   ├── modules/
│   ├── reviews/_template/
│   └── versions/v0.1/
└── harness/
    ├── checklists/
    ├── human-rules/
    ├── process_rules/
    ├── rules/
    └── scripts/
```

| 路径 | 类型 | 职责 | 备注 |
|------|------|------|------|
| `AGENTS.md` | entry | 顶层任务读取顺序与仓库命令 | 增量 retrofit |
| `docs/architecture/repository-workflow.md` | architecture | 工作流模型与验证面 | 仓库级 |
| `docs/architecture/cyfs-gateway-config-domain.md` | architecture | 配置规范任务的领域真相 | overlay |
| `docs/modules/*.md` | module boundary | 长期模块归属与依赖 | 初始种子集 |
| `docs/versions/v0.1/modules/repo-governance/` | module packet | 代表性治理 packet | 当前任务 |
| `harness/rules/*.md` | stable rule | 阶段不变量与门禁 | 持久规则 |
| `harness/process_rules/cyfs-gateway-config-task.md` | process rule | 配置规范任务流程 | 领域专用 |
| `harness/process_rules/implementation-task.md` | process rule | implementation / bugfix 任务流程 | 通用执行 overlay |
| `harness/checklists/cyfs-gateway-config-review-checklist.md` | checklist | 配置任务快速 review 清单 | 领域专用 |
| `harness/human-rules/*.md` | human governance | 协作模式、模块分级与自治边界 | 治理 overlay |
| `harness/scripts/test-run.py` | script | canonical 验证入口 | 非交互 |

## 文档索引
| 文档 | 主题 | 范围 |
|------|------|------|
| `design.md` | 模块设计总览 | full module |

## 风险与回滚
- 如果这套工作流最终显得过重，回滚风险较低，因为当前改动都是增量文档和脚本
- 如果验证命令漂移，需要在同一任务里同步更新模块文档和 `harness/scripts/test-run.py`
