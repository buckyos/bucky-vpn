# PN Server Info Contract Admission

## Implementation Admission Evidence
| evidence_item | source | status | notes |
|---------------|--------|--------|-------|
| proposal_read | docs/versions/v0.1/modules/vpn-frame/proposal.md | pass | Read approved proposal for `CHG-pn-server-info-contract`; it requires `NodeNetwork.pn_server`, selector state, heartbeat state, and SQLite storage to carry structured PN server id, ip, and port. |
| design_read | docs/versions/v0.1/modules/vpn-frame/design.md | pass | Read approved design for `CHG-pn-server-info-contract`; it maps protocol, selector, heartbeat, client connection, HTTP API, and SQLite storage paths. |
| change_scope_matches_request | docs/versions/v0.1/modules/vpn-frame/design.md Directly Mapped Change Items | pass | User clarified that PN server id is the `vpn-server` P2P node id, not endpoint/ip data, and storage must not be endpoint-shaped or old-data compatible; the mapped change directly covers that. |
| active_module_resolved | AGENTS.md and docs/modules/vpn-frame.md | pass | Active version is `v0.1`, module is `vpn-frame`, change_id is `CHG-pn-server-info-contract`. |
| no_chat_only_evidence | proposal/design mapping tables | pass | Admission is based on approved versioned documents and quoted rows below, not chat-only requirements. |

## Document Binding
| doc | sha256 |
|-----|--------|
| docs/versions/v0.1/modules/vpn-frame/proposal.md | a2e96a5784e88c15e44bf9315e4e3e5576fda3bdfb3225b300db3b54e125dbe2 |
| docs/versions/v0.1/modules/vpn-frame/design.md | 5c748d15ef2b0f47cd37a8eb1db33688fb384a79e339fe4da0cce6c3ed5619a3 |

## Coverage Quotes
### Quote: proposal.md CHG-pn-server-info-contract
> | PROP-pn-server-info | CHG-pn-server-info-contract | `NodeNetwork.pn_server`, selector state, heartbeat state, and SQLite PN server storage carry structured PN server id, ip, and port; `id` is the `vpn-server` P2P node id and old endpoint-string data is unsupported. | `vpn-frame/src/vpn_protocol.rs` defines the structured type and direct runtime/storage consumers compile through the vpn-frame harness test entry. |

### Quote: design.md CHG-pn-server-info-contract
> | CHG-pn-server-info-contract | PROP-pn-server-info | Add/use `PnServerInfo`, change protocol, selector, heartbeat, client connection, HTTP API, and SQLite PN server storage so PN server id is the `vpn-server` P2P node id and no endpoint-string storage compatibility remains. | `vpn-frame/src/vpn_protocol.rs`, `vpn-frame/src/server/network_store.rs`, `vpn-frame/src/server/network_manager.rs`, `vpn-frame/src/server/vpn_server.rs`, `vpn-frame/src/client/vpn_server_client.rs`, `vpn-frame/src/client/tunnel_manager.rs`, `vpn-frame/src/client/vpn_client.rs`, `vpn-client/src/p2p_vpn.rs`, `vpn-server/src/server_config.rs`, `vpn-server/src/main.rs`, `vpn-server/src/vpn_control_client.rs`, `vpn-server/src/sqlite_store_factory.rs`, `vpn-server/src/api.rs` |
