---
module: repo-governance
version: v0.1
status: approved
approved_by: user-request
approved_at: 2026-04-22
---

# Repo Governance Testing

## 测试文档索引
| 文档 | 主题 | 范围 |
|------|------|------|
| none | 当前尚未拆分子测试文档 | full module |

## 统一测试入口
- 机器可读计划：`docs/versions/v0.1/modules/repo-governance/testplan.yaml`
- Unit：`python3 ./harness/scripts/test-run.py repo-governance unit`
- DV：`python3 ./harness/scripts/test-run.py repo-governance dv`
- Integration：`python3 ./harness/scripts/test-run.py repo-governance integration`

## 子模块测试
| 子模块 | 职责 | 详细测试文档 | 必须覆盖的行为 | 边界/失败场景 | 测试类型 | 测试文件 |
|--------|------|--------------|----------------|---------------|----------|----------|
| AGENTS 入口 | 稳定任务读取入口 | none | 指向真实的 harness 路径 | 路径引用过期 | 脚本检查 | `harness/scripts/test-run.py` |
| docs 骨架 | 模板和边界文档 | none | 必需文件存在 | packet/模板文件缺失 | 脚本检查 | `harness/scripts/test-run.py` |
| 稳定规则 | 阶段不变量和准入门禁 | none | 必需规则文件存在 | 门禁缺失 | 脚本检查 | `harness/scripts/test-run.py` |
| 领域 overlay | 配置规范工作流 overlay | none | 领域文件存在且与 AGENTS 连通 | overlay 引用丢失 | 脚本检查 | `harness/scripts/test-run.py` |
| 协作治理 | 贡献模式、模块分级和触发规则 | none | `harness/human-rules/` 与触发规则存在且可达 | 自治边界或高风险规则缺失 | 脚本检查 | `harness/scripts/test-run.py` |

## 模块级测试
| 测试项 | 覆盖边界 | 入口 | 预期结果 | 测试类型 | 测试文件/脚本 |
|--------|----------|------|----------|----------|----------------|
| bootstrap 文件存在 | 必需治理骨架 | `python3 ./harness/scripts/test-run.py repo-governance unit` | 所需文件存在时退出码为 0 | 脚本检查 | `harness/scripts/test-run.py` |
| packet 元数据有效 | 代表性模块 packet | `python3 ./harness/scripts/test-run.py repo-governance dv` | front matter 已批准且 testplan 关键字段存在时退出码为 0 | 脚本检查 | `harness/scripts/test-run.py` |
| 交叉引用连通 | AGENTS、docs 与 harness 的连通性 | `python3 ./harness/scripts/test-run.py repo-governance integration` | 引用路径和模块文档对齐时退出码为 0 | 脚本检查 | `harness/scripts/test-run.py` |
| 治理 overlay 连通 | implementation overlay、trigger rules 和 human-rules 已接入入口 | `python3 ./harness/scripts/test-run.py repo-governance integration` | 新增治理工件被入口文档与脚本覆盖时退出码为 0 | 脚本检查 | `harness/scripts/test-run.py` |

## 外部接口测试
| 接口 | 职责 | 成功场景 | 失败/边界场景 | 测试类型 | 测试文档/文件 |
|------|------|----------|---------------|----------|----------------|
| `test-run.py` CLI | canonical 机器可读验证入口 | 已知模块和级别可正常执行 | 未知模块或非法级别返回非零退出码 | 脚本检查 | `harness/scripts/test-run.py` |

## Unit Tests
| 测试项 | 覆盖行为 | 测试文件 |
|--------|----------|----------|
| 必需文件存在性 | 核心 bootstrap 骨架存在 | `harness/scripts/test-run.py` |

## DV Tests
- 校验代表性模块 packet 中存在已批准的元数据
- 校验 `testplan.yaml` 包含 `schema_version`、`module` 和 `levels`
- 校验治理规则至少覆盖 proposal、design、testing、implementation、trigger-based validation 与 acceptance

## Integration Tests
- 校验 `AGENTS.md` 指向真实存在的架构文档和规则文件
- 校验当前仓库模块对应的长期边界文档都存在
- 校验 `repository-workflow.md`、`AGENTS.md` 与 `harness/human-rules/`、`harness/process_rules/implementation-task.md` 的接线一致

## 回归关注点
- 后续重构导致文件引用断裂
- 准入规则与真实模块 packet 布局漂移
- 领域 overlay 文件被删除后，AGENTS 没有同步更新
- 高风险触发规则或模块分级变化后，入口文档和脚本检查没有同步更新

## 完成定义
- [x] Testing 文档覆盖了所有直接子模块，或者明确说明为何没有。
- [x] `testplan.yaml` 与声明的测试入口一致。
- [x] 模块级测试覆盖关键边界行为和失败路径。
- [x] 外部接口具备合同导向的测试说明。
- [ ] 相关自动化验证已经通过。
