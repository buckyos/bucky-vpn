# Task 20260625-proxy-approval-control Admission Evidence

## Implementation Admission Evidence
| evidence_item | source | status | notes |
|---------------|--------|--------|-------|
| proposal_read | docs/versions/v0.1/modules/bucky-vpn-server/proposal.md | pass | Read approved proposal covering SQLite approval persistence and HTTP approval interface for external proxy nodes. |
| design_read | docs/versions/v0.1/modules/bucky-vpn-server/design.md | pass | Read approved design covering `pn_proxy_node` persistence, approved + live selection, and HTTP approval endpoints. |
| change_scope_matches_request | proposal PROP-external-pn-approval-persistence / PROP-external-pn-approval-http-api and design CHG-external-pn-approval-persistence / CHG-external-pn-approval-http-api | pass | The admitted changes directly implement the user's request to store approval state in the database and export HTTP approval APIs. |
| active_module_resolved | docs/versions/v0.1/modules/bucky-vpn-server | pass | The request affects the control node server process, SQLite persistence, selector state, and HTTP control API, all owned by `bucky-vpn-server`. |
| no_chat_only_evidence | versioned docs only | pass | Implementation scope is based on approved proposal/design rows quoted below, not on chat-only assumptions. |

## Document Binding
| doc | sha256 |
|-----|--------|
| docs/versions/v0.1/modules/bucky-vpn-server/proposal.md | 6bc7383840ca99bc7f162823e830fe43d37f76b08d889bd836cced94a5f02ccc |
| docs/versions/v0.1/modules/bucky-vpn-server/design.md | b42efd4289b63d7fc943b09e396d4ca3ba02316c1ea4b896e00635c140f4611d |

## Coverage Quotes

### Quote: proposal.md CHG-external-pn-approval-persistence
> | PROP-external-pn-approval-persistence | CHG-external-pn-approval-persistence | 外部代理节点批准状态持久化到 SQLite，并与 heartbeat liveness 分开参与选择。 | Design 命名持久化 schema、状态枚举、迁移、重启恢复和选择组合规则。 |

### Quote: design.md CHG-external-pn-approval-persistence
> | CHG-external-pn-approval-persistence | PROP-external-pn-approval-persistence | `sqlite-persistence` 持久化外部代理节点批准状态，`control-node-control` 只选择 approved + live 的外部代理节点。 | `vpn-server/src/sqlite_store_factory.rs`, `vpn-server/src/server_config.rs`, `vpn-server/src/main.rs` |

### Quote: proposal.md CHG-external-pn-approval-http-api
> | PROP-external-pn-approval-http-api | CHG-external-pn-approval-http-api | HTTP 控制面导出外部代理节点列表、批准和拒绝接口。 | Design 命名 API 路径、请求/响应模型、权限要求和错误语义。 |

### Quote: design.md CHG-external-pn-approval-http-api
> | CHG-external-pn-approval-http-api | PROP-external-pn-approval-http-api | `http-api` 导出外部代理节点列表、批准和拒绝接口，并复用现有认证边界。 | `vpn-server/src/api.rs`, `vpn-server/src/server_config.rs`, `vpn-server/src/main.rs`, `vpn-server/src/sqlite_store_factory.rs` |
