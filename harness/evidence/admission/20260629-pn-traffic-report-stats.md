# Task 20260629-pn-traffic-report-stats Admission Evidence

## Implementation Admission Evidence
| evidence_item | source | status | notes |
|---------------|--------|--------|-------|
| proposal_read | docs/versions/v0.1/modules/bucky-vpn-server/proposal.md | pass | Read the approved bucky-vpn-server proposal and mapped this bugfix to the traffic statistics database-backed persistence requirement. |
| design_read | docs/versions/v0.1/modules/bucky-vpn-server/design.md | pass | Read the approved bucky-vpn-server design and confirmed `CHG-pn-traffic-db-interface` covers `pn_traffic_service.rs`. |
| change_scope_matches_request | proposal PROP-pn-traffic-db-interface + PROP-external-pn-active-control / design CHG-pn-traffic-db-interface + CHG-external-pn-active-control | pass | The timeout is caused by split proxy PN traffic deltas not reaching the control API statistics path; the fix spans traffic-statistics flush behavior and the external proxy control validation path that observes active PN clients. |
| active_module_resolved | docs/versions/v0.1/modules/bucky-vpn-server | pass | The failing logic is in the server binary's PN traffic reporting/persistence service, not the shared protocol module. |
| no_chat_only_evidence | versioned docs only | pass | Scope and change id are bound to approved proposal/design rows, not chat-only context. |

## Document Binding
| doc | sha256 |
|-----|--------|
| docs/versions/v0.1/modules/bucky-vpn-server/proposal.md | 6bc7383840ca99bc7f162823e830fe43d37f76b08d889bd836cced94a5f02ccc |
| docs/versions/v0.1/modules/bucky-vpn-server/design.md | b42efd4289b63d7fc943b09e396d4ca3ba02316c1ea4b896e00635c140f4611d |

## Coverage Quotes

### Quote: proposal.md CHG-external-pn-active-control
> | PROP-external-pn-active-control | CHG-external-pn-active-control | 外部代理节点主动连接/注册，并在使用前由控制节点策略接受。 | Design 命名主动流程、接受策略、状态 owner、失败行为和验证策略。 |

### Quote: design.md CHG-external-pn-active-control
> | CHG-external-pn-active-control | PROP-external-pn-active-control | `control-node-control` 设计外部代理节点主动连接/注册和接受状态。 | `vpn-server/src/main.rs`, `vpn-server/src/vpn_control_client.rs`, `vpn-server/src/pn_connection_validator.rs` |

### Quote: proposal.md CHG-pn-traffic-db-interface
> | PROP-pn-traffic-db-interface | CHG-pn-traffic-db-interface | 内置代理节点流量统计直接使用现有 database-backed 存储接口。 | Design 命名存储接口，implementation 通过该接口写入流量统计且不新增平行 store。 |

### Quote: design.md CHG-pn-traffic-db-interface
> | CHG-pn-traffic-db-interface | PROP-pn-traffic-db-interface | `traffic-statistics` 通过 `sqlite-persistence` 统一写入累计统计。 | `vpn-server/src/pn_traffic_service.rs`, `vpn-server/src/sqlite_store_factory.rs` |
