---
task_manifest: task.yaml
status: approved
---

# 整理 src 下独立测试文件提案

Risk profile: not-created (created only after high-risk confirmation)

## Workflow Tier Judgment
- Proposed tier: standard
- Final tier: standard
- Tier rationale / triggered boundaries: 本任务横跨三个 Rust crate，不能按 trivial 处理；但只迁移四个测试专用文件并调整 `cfg(test)` 下的文件装载路径，保持原测试模块编译上下文，不修改生产运行时、公共 API、协议、数据、依赖或发布产物，因此当前证据不构成 high-risk 的实质边界。
- Proposal and tier confirmation: 用户于 2026-08-06 回复“确认”，批准本提案及显示的 standard 级别；确认时没有未决问题。

## Background and Goal
项目新规则要求独立测试文件不得位于 `src` 或其子目录，必须位于与对应 `src` 同级的 `tests`。只读盘点发现四个独立测试文件仍位于生产源码树：

- `vpn-client/src/p2p_vpn_pn_registry_tests.rs`
- `vpn-frame/src/client/vpn_client_restart_tests.rs`
- `vpn-server/src/pn_control_client_tests.rs`
- `vpn-server/src/pn_traffic_service_tests.rs`

目标是把这四个文件迁移到各 crate 根目录的 `tests`，同步修正装载路径，并保持现有测试行为与生产构建行为不变。

## Scope
### In scope
- 将客户端 PN registry 测试迁移到 `vpn-client/tests/p2p_vpn_pn_registry_tests.rs`，更新 `p2p_vpn.rs` 的 `include!` 路径及测试内源码快照路径。
- 将共享库 restart 测试迁移到 `vpn-frame/tests/vpn_client_restart_tests.rs`，更新 `vpn_client.rs` 的 `include!` 路径。
- 将服务端 PN control 和 traffic service 测试迁移到 `vpn-server/tests/`，更新对应源码的 `#[path]` 属性。
- 保持四个文件仍由原有 `#[cfg(test)]` 模块装载，使其继续访问既有私有测试边界，不新增生产导出。
- 运行四组测试对应的定向 Cargo 验证，并确认任何 `src/**` 下不再存在独立测试文件。

### Out of scope
- 不移动生产源码文件中的内联 `#[cfg(test)] mod tests`。
- 不重写或重新组织测试逻辑，不新增测试用例，不修改产品运行时行为。
- 不为 `vpn-client` 新建 library target，不扩大任何公开 API 或模块可见性。
- 不重写已完成任务 packet、acceptance report 或 change record 中记录的历史文件路径；这些路径描述的是当时交付证据。
- 不清理当前工作树中的其他无关修改或未跟踪文件。

### Boundary with neighboring modules
三个 crate 分别拥有自己的测试文件和装载引用。本任务使用 `globals` packet 统一表达仓库级整理意图，但不改变 crate 之间的依赖方向或共享合同。`vpn-frame` 仍只向客户端和服务端提供既有生产接口。

## Requirement Review
用户要求与刚新增的项目规则一致，目录目标明确。直接把这些文件改造成 Cargo 外部 integration test 会破坏当前私有符号访问，其中 `vpn-client` 还是 binary-only crate；为迁移而新增公共 API 或 library target 会不必要地扩大生产边界。

更稳妥的方案是只改变文件物理位置：继续从原 `#[cfg(test)]` 模块使用 `include!` 或 `#[path]` 装载 `tests` 下的文件。这样同时满足目录规则和现有私有测试边界。唯一需要同步调整的文件内部相对路径是 `p2p_vpn_pn_registry_tests.rs` 中读取 `p2p_vpn.rs` 的 `include_str!`。

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-relocate-bucky-vpn-tests | 将 bucky-vpn 的独立 PN registry 测试移出 `src`，同时保持原测试模块和行为。 | 仅移动一个测试文件并调整 `p2p_vpn.rs` 装载路径及文件内 `include_str!` 相对路径。 | 测试仍通过 `include!` 编译到原内部模块，而不是成为独立 Cargo integration target。 | PN registry 四个定向测试通过；旧文件不存在；新文件位于 `vpn-client/tests`。 | 不创建 lib target、不改变 PN registry 实现。 |
| P-002 | CHG-relocate-vpn-frame-tests | 将 vpn-frame 的 restart 测试移出 `src/client`。 | 仅移动一个测试文件并调整 `vpn_client.rs` 中的装载路径。 | 保留内部模块上下文以继续验证私有判断函数。 | restart 三个定向测试和同模块既有测试通过；旧文件不存在；新文件位于 `vpn-frame/tests`。 | 不扩大 `is_unchanged_vpn_info_response` 可见性。 |
| P-003 | CHG-relocate-bucky-vpn-server-tests | 将服务端 PN control 与 traffic service 独立测试移出 `src`。 | 移动两个测试文件并修改两个源码文件的 `#[path]`；测试代码和生产接口不变。 | 仍作为原源码模块的测试子模块编译，而不是改造成外部 crate。 | `pn_control_client_tests` 与 `node_traffic_tests` 定向测试通过；旧文件不存在；新文件位于 `vpn-server/tests`。 | 不改流量统计、SQLite、PN 校验或服务端配置行为。 |

## Success Criteria
- Concrete user-visible or system-visible result: 所有已识别独立 Rust 测试文件均位于对应 crate 的 `tests`，`src/**` 只保留生产源码和允许的内联测试块。
- Required evidence: 四个旧路径不存在、四个新路径存在；装载引用全部指向新位置；定向 Cargo 测试通过；最终路径扫描不再发现 `src/**` 下的独立测试文件。
- Explicit non-goals: 不修改测试语义、产品运行时、公共 API、依赖、历史任务文档或无关工作树内容。

## Risks
- Rust `include!`/`#[path]` 的相对路径如果写错会造成测试 target 编译失败，必须以对应 crate 的定向测试验证。
- `p2p_vpn_pn_registry_tests.rs` 内部还有一个 `include_str!` 相对路径，移动后必须同步更新，否则源码顺序断言无法编译或读取错误文件。
- 两个待迁移文件和其父源码当前包含用户未提交改动；迁移必须保留现有内容，只做路径相关差异，并依赖 lower-tier baseline 区分本任务与既有工作树状态。
- 已完成任务文档会继续引用旧路径作为历史证据；修改这些历史记录反而会歪曲当时状态，因此明确不更新。
