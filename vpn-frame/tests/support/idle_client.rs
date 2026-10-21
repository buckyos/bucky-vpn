mod tunnel_activity {
    include!("../../src/client/tunnel_activity.rs");
    mod idle_unit {
        include!("idle_unit.rs");
    }
}
#[path = "../../src/client/tunnel_manager.rs"]
mod tunnel_manager;
#[path = "../../src/client/packet_dispatcher.rs"]
mod packet_dispatcher;

mod idle_dv;
