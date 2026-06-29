# Implementation Admission: proxy-node-control

## Implementation Admission Evidence
| evidence_item | source | status | notes |
| --- | --- | --- | --- |
| proposal_read | `docs/versions/v0.1/modules/bucky-vpn-server/proposal.md` | pass | 已读取 approved proposal，覆盖控制节点/代理节点配置、注册、心跳、内置默认允许和统计持久化接口复用。 |
| design_read | `docs/versions/v0.1/modules/bucky-vpn-server/design.md` | pass | 已读取 approved design，`Directly Mapped Change Items` 覆盖全部 admitted change_id。 |
| change_scope_matches_request | proposal/design mapping | pass | 当前请求“自动完成后续步骤”映射到代理节点配置、控制节点接受、心跳、内置默认允许和统计接口复用的所有 change_id。 |
| active_module_resolved | `bucky-vpn-server` packet | pass | active version `v0.1`，active module `bucky-vpn-server`。 |
| no_chat_only_evidence | proposal/design approved docs | pass | 不使用聊天上下文作为 implementation evidence；以 approved proposal/design 的 hash 和 quote 绑定为准。 |

## Document Binding
| doc | sha256 |
| --- | --- |
| docs/versions/v0.1/modules/bucky-vpn-server/proposal.md | e5c281e62918e665c680b4b9177778e101a658e6d7e27fa1b5f87f793a654aab |
| docs/versions/v0.1/modules/bucky-vpn-server/design.md | f0173db8a818710f574460c360c0870714bd91b60438ad986e087e50e45c7eca |

## Coverage Quotes
### Quote: proposal.md CHG-pn-config-no-static-addresses
> | PROP-pn-config-no-static-addresses | CHG-pn-config-no-static-addresses | 从目标代理节点配置合同中移除 `pn.server_addresses`。 | Proposal/design/code 不再记录或要求静态外部代理节点地址列表。 |

### Quote: design.md CHG-pn-config-no-static-addresses
> | CHG-pn-config-no-static-addresses | PROP-pn-config-no-static-addresses | `server-config` 移除静态外部代理地址合同，`process-assembly` 不再注入额外 endpoint。 | `vpn-server/src/server_config.rs`, `vpn-server/src/main.rs` |

### Quote: proposal.md CHG-external-pn-active-control
> | PROP-external-pn-active-control | CHG-external-pn-active-control | 外部代理节点主动连接/注册，并在使用前由控制节点策略接受。 | Design 命名主动流程、接受策略、状态 owner、失败行为和验证策略。 |

### Quote: design.md CHG-external-pn-active-control
> | CHG-external-pn-active-control | PROP-external-pn-active-control | `control-node-control` 设计外部代理节点主动连接/注册和接受状态。 | `vpn-server/src/main.rs`, `vpn-server/src/vpn_control_client.rs`, `vpn-server/src/pn_connection_validator.rs` |

### Quote: proposal.md CHG-pure-pn-sn-address
> | PROP-pure-pn-sn-address | CHG-pure-pn-sn-address | 纯代理节点启动配置包含连接 control/bootstrap server 所需的控制节点地址。 | Design 命名 YAML 字段、地址格式、验证行为以及纯代理节点如何使用它。 |

### Quote: design.md CHG-pure-pn-sn-address
> | CHG-pure-pn-sn-address | PROP-pure-pn-sn-address | `server-config` 提供纯代理节点控制节点地址配置，区别于 removed static proxy address list。 | `vpn-server/src/server_config.rs`, `vpn-server/src/vpn_control_client.rs`, `vpn-server/src/main.rs` |

### Quote: proposal.md CHG-pn-sn-heartbeat
> | PROP-pn-sn-heartbeat | CHG-pn-sn-heartbeat | 代理节点与控制节点在连接/注册后保持心跳，heartbeat liveness 控制代理节点可用性。 | Design 命名心跳间隔、超时、状态迁移、重连行为和选择影响。 |

### Quote: design.md CHG-pn-sn-heartbeat
> | CHG-pn-sn-heartbeat | PROP-pn-sn-heartbeat | `control-node-control` 定义 heartbeat liveness、timeout 和恢复对选择的影响。 | `vpn-server/src/vpn_control_client.rs`, `vpn-server/src/main.rs` |

### Quote: proposal.md CHG-colocated-pn-default-allowed
> | PROP-colocated-pn-default-allowed | CHG-colocated-pn-default-allowed | 与控制节点同进程的内置代理节点在本地代理启用时默认允许。 | Design 区分同进程内置代理节点权限和外部代理节点接受流程，并保持无配置默认行为。 |

### Quote: design.md CHG-colocated-pn-default-allowed
> | CHG-colocated-pn-default-allowed | PROP-colocated-pn-default-allowed | `process-assembly` 对内置代理节点默认允许，仍由 `pn.enabled` 控制。 | `vpn-server/src/main.rs`, `vpn-server/src/server_config.rs` |

### Quote: proposal.md CHG-pn-traffic-db-interface
> | PROP-pn-traffic-db-interface | CHG-pn-traffic-db-interface | 内置代理节点流量统计直接使用现有 database-backed 存储接口。 | Design 命名存储接口，implementation 通过该接口写入流量统计且不新增平行 store。 |

### Quote: design.md CHG-pn-traffic-db-interface
> | CHG-pn-traffic-db-interface | PROP-pn-traffic-db-interface | `traffic-statistics` 通过 `sqlite-persistence` 统一写入累计统计。 | `vpn-server/src/pn_traffic_service.rs`, `vpn-server/src/sqlite_store_factory.rs` |

### Quote: proposal.md CHG-local-pn-toggle-preserved
> | PROP-local-pn-toggle-preserved | CHG-local-pn-toggle-preserved | `pn.enabled` 继续控制内置本地代理节点。 | 缺少配置时仍默认启动本地代理节点，显式关闭时阻止本地代理节点启动。 |

### Quote: design.md CHG-local-pn-toggle-preserved
> | CHG-local-pn-toggle-preserved | PROP-local-pn-toggle-preserved | `server-config` 和 `process-assembly` 保持 `pn.enabled` 的本地内置代理节点开关语义。 | `vpn-server/src/server_config.rs`, `vpn-server/src/main.rs` |
