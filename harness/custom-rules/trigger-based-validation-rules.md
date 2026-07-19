# 变更触发验证规则

## 目标
- 为高风险改动定义必须追加读取、验证或评审关注的公共触发规则

## 适用范围
- implementation、bugfix、review、acceptance 任务
- 触及共享协议、接口合同、配置规范、打包流程或仓库治理工件的改动

## 通用规则
- 命中触发条件后，不能只按默认模块 packet 执行；必须补读本规则声明的额外文档
- 如果触发项会改变验证面，必须先回写对应模块的 `testing.md` 和 `testplan.yaml`，再进入实现或验收
- 如果高成本验证本轮未执行，testing 和 acceptance 都必须明确记录未执行原因，而不是默认通过

## 触发矩阵
| 触发条件 | 典型路径 | 必须追加动作 | Acceptance 必查项 |
|----------|----------|--------------|-------------------|
| 共享协议/运行时合同改动 | `vpn-frame/**`、`docs/modules/vpn-frame.md` | 补读 `docs/modules/vpn-frame.md`、相邻客户端/服务端模块 packet；在 testing 中声明受影响模块和合同边界 | 客户端与服务端是否都被纳入影响分析；合同变更是否能映射到直接 testing 项 |
| Web API 模型或序列化形状改动 | `vpn_web/lib/api.dart`、`vpn_web/lib/api.g.dart`、后端返回字段合同 | 补读 `docs/modules/vpn_web.md` 与 `harness/custom-rules/vpn-web-no-new-tests-rule.md`；若注解变更，testing 必须声明 build_runner 再生成要求 | `@JsonKey` 与后端字段是否对齐；生成文件是否由工具产出；是否错误地绕开了前端不新增测试例外 |
| `cyfs-gateway` 配置规范或 process-chain 说明改动 | `docs/architecture/cyfs-gateway-config-domain.md`、`harness/process_rules/cyfs-gateway-config-task.md`、相关文档示例 | 补读 `harness/custom-rules/cyfs-gateway-config-spec-rules.md` 与 checklist；review/acceptance 必须使用领域清单 | 运行时真相、类型注册集、DSL canonical 命令名是否仍然一致 |
| 打包脚本或发布入口改动 | `build_*.sh`、`build_win.bat` | 在 testing 中显式记录目标平台、可执行命令和未执行平台原因 | 平台差异是否被记录；未执行的平台验证是否被清楚标注为待补证据 |
| 仓库治理规则或验证入口改动 | `AGENTS.md`、`docs/architecture/repository-workflow.md`、`docs/versions/v0.1/modules/repo-governance/**`、`harness/**` | 按 `repo-governance` packet 执行；必须更新对应治理文档和 `python3 ./harness/scripts/test-run.py repo-governance <level>` 的覆盖面 | 新规则是否被入口文档引用；脚本检查是否已经覆盖新增治理工件 |

## 失败处理
- 命中触发条件但没有补充 testing 覆盖：退回 testing
- 命中触发条件但实现直接开始：退回 implementation
- 命中触发条件但 acceptance 没有检查额外风险：判定 acceptance 失败
