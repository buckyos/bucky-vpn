---
module: example-module
version: v0.1
status: draft
approved_by:
approved_at:
---

# [模块名] Testing

## 测试文档索引
| 文档 | 主题 | 范围 |
|------|------|------|
| none | 当前尚未拆分子测试文档 | full module |

## 统一测试入口
- 机器可读计划：`docs/versions/v0.1/modules/<module>/testplan.yaml`
- Unit：`python3 ./harness/scripts/test-run.py <module> unit`
- DV：`python3 ./harness/scripts/test-run.py <module> dv`
- Integration：`python3 ./harness/scripts/test-run.py <module> integration`

## 子模块测试
| 子模块 | 职责 | 详细测试文档 | 必须覆盖的行为 | 边界/失败场景 | 测试类型 | 测试文件 |
|--------|------|--------------|----------------|---------------|----------|----------|
| | | | | | | |

## 模块级测试
| 测试项 | 覆盖边界 | 入口 | 预期结果 | 测试类型 | 测试文件/脚本 |
|--------|----------|------|----------|----------|----------------|
| | | | | | |

## 外部接口测试
| 接口 | 职责 | 成功场景 | 失败/边界场景 | 测试类型 | 测试文档/文件 |
|------|------|----------|---------------|----------|----------------|
| | | | | | |

## Unit Tests
| Test Item | Covered Behavior | Test File |
|-----------|------------------|-----------|
| | | |

## DV Tests
<!-- 单模块可运行验证 -->

## Integration Tests
<!-- 邻接模块合同与协作验证 -->

## 回归关注点
<!-- 历史缺陷和高风险边界 -->

## 完成定义
- [ ] Testing 文档覆盖了所有直接子模块，或者明确说明为何没有。
- [ ] `testplan.yaml` 与文档中声明的测试入口一致。
- [ ] 模块级测试覆盖关键边界行为和失败路径。
- [ ] 外部接口具备合同导向的测试说明。
- [ ] 相关自动化验证已经通过。
