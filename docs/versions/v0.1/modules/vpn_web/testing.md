---
module: vpn_web
version: v0.1
status: approved
approved_by: user-request
approved_at: 2026-04-16
---

# vpn_web Testing

## 测试文档索引
| 文档 | 主题 | 范围 |
|------|------|------|
| none | 当前尚未拆分子测试文档 | full module |

## 统一测试入口
- 机器可读计划：`docs/versions/v0.1/modules/vpn_web/testplan.yaml`
- Unit：`python3 ./harness/scripts/test-run.py vpn_web unit`
- DV：`python3 ./harness/scripts/test-run.py vpn_web dv`
- Integration：`python3 ./harness/scripts/test-run.py vpn_web integration`

## 模块例外
- 仓库规则：默认不要新增 `vpn_web` 测试用例，除非用户明确要求例外。
- 现有前端验证优先使用 `flutter analyze`、`flutter build web` 和有针对性的人工验证。
- 现有遗留测试可以作为信号运行，但不构成必须扩展测试套件的理由。

## 子模块测试
| 子模块 | 职责 | 详细测试文档 | 必须覆盖的行为 | 边界/失败场景 | 测试类型 | 测试文件 |
|--------|------|--------------|----------------|---------------|----------|----------|
| `main.dart` | 应用根和路由 | none | 应用能启动且路由能渲染 | 路由不匹配、主题或启动失败 | analyze/build/manual verification | `vpn_web/test/widget_test.dart` 仅保留为遗留覆盖 |
| pages and dialogs | UI 流程 | none | 核心导航和编辑流程能正确渲染 | 非法表单输入、空数据、动作失败 | analyze/build/manual verification | 默认不新增测试 |
| `api.dart` plus `api.g.dart` | 类型化 API/数据模型层 | none | 模型序列化和反序列化与后端 key 对齐 | 生成代码过期、字段不匹配 | analyzer plus generated-code rebuild when changed | 生成文件边界 |
| `http_client.dart` | 后端请求封装 | none | 请求和结果封装行为一致 | 网络或 API 失败处理 | analyze/build/manual verification | 默认不新增测试 |

## 模块级测试
| 测试项 | 覆盖边界 | 入口 | 预期结果 | 测试类型 | 测试文件/脚本 |
|--------|----------|------|----------|----------|----------------|
| 现有 Flutter 测试套件 | 仅作为遗留 widget 测试信号 | `python3 ./harness/scripts/test-run.py vpn_web unit` | 如果仓库仍保留现有 `flutter test`，则它可以运行 | automated | `harness/scripts/test-run.py` |
| 静态分析 | 前端编译与 lint 面 | `python3 ./harness/scripts/test-run.py vpn_web dv` | `flutter analyze` 退出码为 0 | automated | `harness/scripts/test-run.py` |
| Web 构建 | 应用外壳、路由和生成代码的集成结果 | `python3 ./harness/scripts/test-run.py vpn_web integration` | `flutter build web` 退出码为 0 | automated | `harness/scripts/test-run.py` |

## 外部接口测试
| 接口 | 职责 | 成功场景 | 失败/边界场景 | 测试类型 | 测试文档/文件 |
|------|------|----------|---------------|----------|----------------|
| backend API contract usage | 类型化前端数据边界 | 模型和 wrapper 与后端 key 保持一致 | 生成代码过期或合同漂移 | analyzer/build and manual verification | `vpn_web/lib/api.dart` |

## Unit Tests
| 测试项 | 覆盖行为 | 测试文件 |
|--------|----------|----------|
| 现有 widget smoke test | 当前仍是 Flutter 脚手架遗留的 counter 风格 smoke test；默认不扩展它 | `vpn_web/test/widget_test.dart` |

## DV Tests
- 运行 `flutter analyze`。

## Integration Tests
- 运行 `flutter build web`。

## 回归关注点
- 现有 widget 测试与真实路由应用不一致
- `api.dart` 与 `api.g.dart` 之间的生成模型漂移
- UI 编辑后出现路由和弹窗回归
- 违反仓库规则，误新增新的前端测试用例

## 完成定义
- [x] Testing 文档覆盖了所有直接子模块，或者明确说明为何没有。
- [x] `testplan.yaml` 与声明的测试入口一致。
- [x] 模块级测试覆盖关键边界行为和失败路径。
- [x] 外部接口具备合同导向的测试说明。
- [x] 除非用户明确要求例外，否则没有新增 `vpn_web` 测试用例。
- [ ] 相关自动化验证已经通过。
