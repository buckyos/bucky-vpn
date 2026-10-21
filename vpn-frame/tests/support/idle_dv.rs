use super::packet_dispatcher::{PacketDispatcher, PacketDispatcherConfig};
use super::tunnel_manager::{TunnelManager, TunnelPkgRecv};
use crate::errors::{VpnErrorCode, VpnResult, vpn_err};
use crate::server::{NetworkGroupId, NetworkId, NetworkMember, NodeId};
use crate::{DataHeader, VpnTunnelFactory, VpnTunnelListener, VpnTunnelRecv, VpnTunnelSend};
use bucky_raw_codec::{RawConvertTo, RawFixedBytes, RawFrom};
use std::io::{Error, ErrorKind};
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::{Arc, atomic::{AtomicBool, AtomicUsize, Ordering}};
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, DuplexStream, ReadBuf, ReadHalf, WriteHalf};
use tokio::sync::{Mutex, mpsc};

const IDLE: Duration = Duration::from_millis(120);
const TICK: Duration = Duration::from_millis(10);
const WAIT: Duration = Duration::from_secs(2);

#[derive(Default)]
struct Drops {
    send: AtomicUsize,
    recv: AtomicUsize,
    read_polls: AtomicUsize,
    blocked: AtomicBool,
    pending_polled: AtomicBool,
    blocked_flush: AtomicBool,
    pending_flush: AtomicBool,
    fail_write: AtomicBool,
    fail_read: AtomicBool,
    closed: AtomicBool,
}

struct Send { io: WriteHalf<DuplexStream>, drops: Arc<Drops>, node: NodeId }
struct Recv { io: ReadHalf<DuplexStream>, drops: Arc<Drops> }
impl Drop for Send {
    fn drop(&mut self) { self.drops.send.fetch_add(1, Ordering::SeqCst); }
}
impl Drop for Recv {
    fn drop(&mut self) { self.drops.recv.fetch_add(1, Ordering::SeqCst); }
}
impl VpnTunnelSend for Send {
    fn is_target_tunnel(&self, target: &NodeId) -> bool { &self.node == target }
    fn is_closed(&self) -> bool { self.drops.closed.load(Ordering::SeqCst) }
}
impl VpnTunnelRecv for Recv {}
impl AsyncRead for Recv {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<Result<(), Error>> {
        self.drops.read_polls.fetch_add(1, Ordering::SeqCst);
        if self.drops.fail_read.load(Ordering::SeqCst) {
            return Poll::Ready(Err(ErrorKind::ConnectionReset.into()));
        }
        Pin::new(&mut self.io).poll_read(cx, buf)
    }
}
impl AsyncWrite for Send {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize, Error>> {
        if self.drops.fail_write.load(Ordering::SeqCst) {
            return Poll::Ready(Err(ErrorKind::BrokenPipe.into()));
        }
        if self.drops.blocked.load(Ordering::SeqCst) {
            self.drops.pending_polled.store(true, Ordering::SeqCst);
            return Poll::Pending;
        }
        let result = Pin::new(&mut self.io).poll_write(cx, buf);
        if result.is_pending() {
            self.drops.pending_polled.store(true, Ordering::SeqCst);
        }
        result
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        if self.drops.blocked_flush.load(Ordering::SeqCst) {
            self.drops.pending_flush.store(true, Ordering::SeqCst);
            return Poll::Pending;
        }
        Pin::new(&mut self.io).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        Pin::new(&mut self.io).poll_shutdown(cx)
    }
}

struct Peer {
    io: DuplexStream,
    drops: Arc<Drops>,
    group: NetworkGroupId,
    network: NetworkId,
    node: NodeId,
}
fn pair(group: NetworkGroupId, network: NetworkId, node: NodeId) -> ((Recv, Send), Peer) {
    let (local, remote) = tokio::io::duplex(65536);
    let (read, write) = tokio::io::split(local);
    let drops = Arc::new(Drops::default());
    ((Recv { io: read, drops: drops.clone() }, Send { io: write, drops: drops.clone(), node: node.clone() }),
        Peer { io: remote, drops, group, network, node })
}

struct Factory {
    peers: mpsc::UnboundedSender<Peer>,
    attempts: AtomicUsize,
    fail: AtomicBool,
    dead: AtomicBool,
}
#[async_trait::async_trait]
impl VpnTunnelFactory<Recv, Send> for Factory {
    async fn create_tunnel(&self, group: NetworkGroupId, network: NetworkId, node: &NodeId) -> VpnResult<(Recv, Send)> {
        self.attempts.fetch_add(1, Ordering::SeqCst);
        if self.fail.load(Ordering::SeqCst) { return Err(vpn_err!(VpnErrorCode::Failed, "injected create failure")); }
        let (channel, peer) = pair(group, network, node.clone());
        peer.drops.closed.store(self.dead.load(Ordering::SeqCst), Ordering::SeqCst);
        self.peers.send(peer).unwrap_or_else(|_| panic!("peer receiver dropped"));
        Ok(channel)
    }
}
struct Listener { accepted: Mutex<mpsc::UnboundedReceiver<(Recv, Send)>> }
#[async_trait::async_trait]
impl VpnTunnelListener<Recv, Send> for Listener {
    async fn accept(&self) -> VpnResult<(Recv, Send)> {
        self.accepted.lock().await.recv().await.ok_or_else(|| vpn_err!(VpnErrorCode::Failed, "listener closed"))
    }
}
struct Packages { tx: mpsc::UnboundedSender<(NetworkId, Vec<u8>)> }
#[async_trait::async_trait]
impl TunnelPkgRecv for Packages {
    async fn on_recv(&self, network: NetworkId, data: Vec<u8>) -> VpnResult<()> {
        let _ = self.tx.send((network, data));
        Ok(())
    }
}
type Manager = TunnelManager<Recv, Send, Factory, Listener>;
struct Harness {
    manager: Arc<Manager>,
    factory: Arc<Factory>,
    peers: mpsc::UnboundedReceiver<Peer>,
    accepted: mpsc::UnboundedSender<(Recv, Send)>,
    received: mpsc::UnboundedReceiver<(NetworkId, Vec<u8>)>,
}
impl Harness {
    fn new() -> Self {
        let (peers_tx, peers) = mpsc::unbounded_channel();
        let factory = Arc::new(Factory { peers: peers_tx, attempts: AtomicUsize::new(0), fail: AtomicBool::new(false), dead: AtomicBool::new(false) });
        let (accepted, accepted_rx) = mpsc::unbounded_channel();
        let listener = Arc::new(Listener { accepted: Mutex::new(accepted_rx) });
        let (tx, received) = mpsc::unbounded_channel();
        let packages = Arc::new(Packages { tx });
        #[cfg(not(feature = "idle_baseline"))]
        let manager = Arc::new(Manager::new_with_idle_policy(factory.clone(), listener, packages, IDLE, TICK));
        #[cfg(feature = "idle_baseline")]
        let manager = Arc::new(Manager::new(factory.clone(), listener, packages));
        Self { manager, factory, peers, accepted, received }
    }
    fn route(&self, group: u64, network: u64, suffix: u8) -> IpAddr {
        let ip: IpAddr = format!("10.0.0.{suffix}").parse().unwrap();
        self.manager.get_router().add_network(group, network, None, vec![NetworkMember {
            id: NodeId::from(&[suffix][..]), ip: ip.to_string(), ipv6: None,
        }]);
        ip
    }
    fn dispatcher(&self) -> Arc<PacketDispatcher<Recv, Send, Factory, Listener>> {
        PacketDispatcher::new(self.manager.clone(), PacketDispatcherConfig {
            shards_per_target: 1, max_batch_pkts: 1, reconnect_backoff_min_ms: 10,
            reconnect_backoff_max_ms: 40, ..PacketDispatcherConfig::default()
        })
    }
    async fn next_peer(&mut self) -> Peer { tokio::time::timeout(WAIT, self.peers.recv()).await.unwrap().unwrap() }
}
async fn until(mut predicate: impl FnMut() -> bool) {
    tokio::time::timeout(WAIT, async {
        while !predicate() { tokio::time::sleep(Duration::from_millis(2)).await; }
    }).await.expect("lifecycle predicate timed out");
}
async fn read_packet(peer: &mut Peer) -> (u64, Vec<u8>) {
    tokio::time::timeout(WAIT, async {
        let mut header = vec![0; DataHeader::raw_bytes().unwrap()];
        peer.io.read_exact(&mut header).await.unwrap();
        let header = DataHeader::clone_from_slice(&header).unwrap();
        let mut data = vec![0; header.pkg_len as usize];
        peer.io.read_exact(&mut data).await.unwrap();
        (header.network_id, data)
    }).await.unwrap()
}
async fn inbound(peer: &mut Peer, network: u64, data: &[u8]) {
    let header = DataHeader { network_id: network, pkg_len: data.len() as u16 }.to_vec().unwrap();
    peer.io.write_all(&header).await.unwrap();
    peer.io.write_all(data).await.unwrap();
}
async fn released(drops: &Arc<Drops>) {
    until(|| drops.send.load(Ordering::SeqCst) == 1 && drops.recv.load(Ordering::SeqCst) == 1).await;
}

#[tokio::test]
async fn dispatcher_held_idle_channel_is_reclaimed_and_first_packet_reconnects() {
    let mut h = Harness::new();
    let ip = h.route(1, 7, 2);
    let dispatcher = h.dispatcher();
    dispatcher.dispatch(1, 7, ip, b"first", false).await.unwrap();
    let mut peer = h.next_peer().await;
    assert_eq!(read_packet(&mut peer).await, (7, b"first".to_vec()));
    tokio::time::sleep(IDLE + TICK * 3).await;
    assert_eq!(peer.drops.send.load(Ordering::SeqCst), 1, "idle-held-sender-not-reclaimed");
    released(&peer.drops).await;
    dispatcher.dispatch(1, 7, ip, b"after-idle", false).await.unwrap();
    let mut replacement = h.next_peer().await;
    assert_eq!(read_packet(&mut replacement).await, (7, b"after-idle".to_vec()));
    assert_eq!(h.factory.attempts.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn pooled_and_accepted_pending_resources_expire_without_lookups() {
    let mut h = Harness::new();
    let ip = h.route(1, 7, 2);
    let guard = h.manager.get_send(1, 7, ip).await.unwrap();
    let pooled = h.next_peer().await;
    drop(guard);
    let (accepted, pending) = pair(1, 7, NodeId::from(&[3][..]));
    h.accepted.send(accepted).unwrap_or_else(|_| panic!("accept loop stopped"));
    until(|| pending.drops.read_polls.load(Ordering::SeqCst) > 0).await;
    assert_eq!(pending.drops.send.load(Ordering::SeqCst), 0);
    released(&pooled.drops).await;
    released(&pending.drops).await;
    assert_eq!(h.factory.attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn receive_only_traffic_and_partial_frames_keep_channel_alive() {
    let mut h = Harness::new();
    let ip = h.route(1, 7, 2);
    let guard = h.manager.get_send(1, 7, ip).await.unwrap();
    let mut peer = h.next_peer().await;
    drop(guard);
    let frame = DataHeader { network_id: 7, pkg_len: 8 }.to_vec().unwrap();
    let bytes = [frame.as_slice(), b"inbound!"].concat();
    for byte in bytes {
        peer.io.write_all(&[byte]).await.unwrap();
        tokio::time::sleep(Duration::from_millis(25)).await;
        assert_eq!(peer.drops.send.load(Ordering::SeqCst), 0);
    }
    assert_eq!(tokio::time::timeout(WAIT, h.received.recv()).await.unwrap().unwrap(), (7, b"inbound!".to_vec()));
    inbound(&mut peer, 7, b"more").await;
    assert_eq!(tokio::time::timeout(WAIT, h.received.recv()).await.unwrap().unwrap(), (7, b"more".to_vec()));
    released(&peer.drops).await;
}

#[tokio::test]
#[cfg(not(feature = "idle_baseline"))]
async fn pending_channel_reuses_target_and_active_receive_survives_acquisition() {
    let h = Harness::new();
    let ip = h.route(1, 7, 2);
    let (accepted, mut peer) = pair(1, 7, NodeId::from(&[2][..]));
    h.accepted.send(accepted).unwrap_or_else(|_| panic!("listener stopped"));
    until(|| peer.drops.read_polls.load(Ordering::SeqCst) > 0).await;
    let mut guard = h.manager.get_send(1, 7, ip).await.unwrap();
    let _permit = guard.begin_send().unwrap();
    guard.write_all(b"reuse").await.unwrap();
    guard.flush().await.unwrap();
    let mut bytes = [0; 5];
    peer.io.read_exact(&mut bytes).await.unwrap();
    assert_eq!(&bytes, b"reuse");
    assert_eq!(h.factory.attempts.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn receiver_eof_read_failure_and_write_failure_release_then_recover() {
    for failure in [0, 1, 2] {
        let mut h = Harness::new();
        let ip = h.route(1, 7, 2);
        let dispatcher = h.dispatcher();
        dispatcher.dispatch(1, 7, ip, b"before", false).await.unwrap();
        let mut peer = h.next_peer().await;
        assert_eq!(read_packet(&mut peer).await.1, b"before");
        match failure {
            0 => peer.io.shutdown().await.unwrap(),
            1 => {
                peer.drops.fail_read.store(true, Ordering::SeqCst);
                peer.io.write_all(&[0]).await.unwrap();
            }
            _ => {
                peer.drops.fail_write.store(true, Ordering::SeqCst);
                dispatcher.dispatch(1, 7, ip, b"fails", false).await.unwrap();
            }
        }
        released(&peer.drops).await;
        dispatcher.dispatch(1, 7, ip, b"recover", false).await.unwrap();
        let mut replacement = h.next_peer().await;
        assert_eq!(read_packet(&mut replacement).await.1, b"recover");
    }
}

#[tokio::test]
async fn blocked_send_survives_deadline_then_cancelled_shard_can_expire() {
    let mut h = Harness::new();
    let ip = h.route(1, 7, 2);
    let guard = h.manager.get_send(1, 7, ip).await.unwrap();
    let peer = h.next_peer().await;
    peer.drops.blocked.store(true, Ordering::SeqCst);
    drop(guard);
    let dispatcher = h.dispatcher();
    dispatcher.dispatch(1, 7, ip, b"blocked", false).await.unwrap();
    until(|| peer.drops.pending_polled.load(Ordering::SeqCst)).await;
    tokio::time::sleep(IDLE * 2).await;
    assert_eq!(peer.drops.send.load(Ordering::SeqCst), 0);
    drop(dispatcher);
    released(&peer.drops).await;
}

#[tokio::test]
async fn fresh_dead_workers_use_backoff_and_preserve_batch_until_recovery() {
    let mut h = Harness::new();
    let ip = h.route(1, 7, 2);
    h.factory.dead.store(true, Ordering::SeqCst);
    let dispatcher = h.dispatcher();
    dispatcher.dispatch(1, 7, ip, b"retained", false).await.unwrap();
    tokio::time::sleep(Duration::from_millis(90)).await;
    let attempts = h.factory.attempts.load(Ordering::SeqCst);
    assert!((2..=6).contains(&attempts), "acquisition must back off: {attempts}");
    h.factory.dead.store(false, Ordering::SeqCst);
    loop {
        let mut peer = h.next_peer().await;
        if !peer.drops.closed.load(Ordering::SeqCst) {
            assert_eq!(read_packet(&mut peer).await.1, b"retained");
            break;
        }
    }
}

#[tokio::test]
async fn create_failure_backoff_recovers_on_next_packet() {
    let mut h = Harness::new();
    let ip = h.route(1, 7, 2);
    h.factory.fail.store(true, Ordering::SeqCst);
    let dispatcher = h.dispatcher();
    dispatcher.dispatch(1, 7, ip, b"fails", false).await.unwrap();
    until(|| h.factory.attempts.load(Ordering::SeqCst) > 0).await;
    tokio::time::sleep(Duration::from_millis(30)).await;
    h.factory.fail.store(false, Ordering::SeqCst);
    dispatcher.dispatch(1, 7, ip, b"retry", false).await.unwrap();
    let mut peer = h.next_peer().await;
    assert_eq!(read_packet(&mut peer).await.1, b"retry");
}

#[tokio::test]
async fn networks_targets_ordering_and_shutdown_remain_isolated() {
    let mut h = Harness::new();
    let ip = h.route(1, 7, 2);
    let other_ip = h.route(2, 8, 3);
    let dispatcher = h.dispatcher();
    dispatcher.dispatch(1, 7, ip, b"a", false).await.unwrap();
    dispatcher.dispatch(2, 8, other_ip, b"b", false).await.unwrap();
    let mut first = h.next_peer().await;
    let mut second = h.next_peer().await;
    if first.group != 1 { std::mem::swap(&mut first, &mut second); }
    assert_eq!((first.group, first.network, first.node.clone()), (1, 7, NodeId::from(&[2][..])));
    assert_eq!((second.group, second.network, second.node.clone()), (2, 8, NodeId::from(&[3][..])));
    assert_eq!(read_packet(&mut first).await, (7, b"a".to_vec()));
    assert_eq!(read_packet(&mut second).await, (8, b"b".to_vec()));
    for _ in 0..8 {
        inbound(&mut second, 8, b"active").await;
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    released(&first.drops).await;
    assert_eq!(second.drops.send.load(Ordering::SeqCst), 0);
    for packet in [b"one", b"two", b"end"] {
        dispatcher.dispatch(2, 8, other_ip, packet, false).await.unwrap();
    }
    for packet in [b"one", b"two", b"end"] {
        assert_eq!(read_packet(&mut second).await, (8, packet.to_vec()));
    }
    drop(dispatcher);
    let weak = Arc::downgrade(&h.manager);
    let Harness { manager, factory, peers, accepted, received } = h;
    drop(manager);
    released(&second.drops).await;
    until(|| weak.upgrade().is_none()).await;
    assert!(accepted.is_closed());
    drop((factory, peers, received));
}

#[tokio::test]
async fn receive_eof_during_real_write_backpressure_reconnects_for_next_packet() {
    let mut h = Harness::new();
    let ip = h.route(1, 7, 2);
    let dispatcher = h.dispatcher();
    dispatcher.dispatch(1, 7, ip, b"first", false).await.unwrap();
    let mut peer = h.next_peer().await;
    assert_eq!(read_packet(&mut peer).await.1, b"first");
    // The frame header plus the maximum payload fills the 65536-byte real pipe.
    dispatcher.dispatch(1, 7, ip, &vec![42; u16::MAX as usize], false).await.unwrap();
    until(|| peer.drops.pending_polled.load(Ordering::SeqCst)).await;
    peer.io.shutdown().await.unwrap();
    released(&peer.drops).await;
    dispatcher.dispatch(1, 7, ip, b"after-eof", false).await.unwrap();
    let mut replacement = tokio::time::timeout(WAIT, h.peers.recv())
        .await.expect("closed-writer-did-not-reconnect").unwrap();
    assert_eq!(read_packet(&mut replacement).await, (7, b"after-eof".to_vec()));
    assert_eq!(h.factory.attempts.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn pending_flush_reconnects_after_receive_or_transport_closes() {
    for failure in 0..3 {
        let mut h = Harness::new();
        let ip = h.route(1, 7, 2);
        let guard = h.manager.get_send(1, 7, ip).await.unwrap();
        let mut peer = h.next_peer().await;
        peer.drops.blocked_flush.store(true, Ordering::SeqCst);
        drop(guard);
        let dispatcher = h.dispatcher();
        dispatcher.dispatch(1, 7, ip, b"before", false).await.unwrap();
        until(|| peer.drops.pending_flush.load(Ordering::SeqCst)).await;
        assert_eq!(read_packet(&mut peer).await.1, b"before");
        match failure {
            0 => peer.io.shutdown().await.unwrap(),
            1 => {
                peer.drops.fail_read.store(true, Ordering::SeqCst);
                peer.io.write_all(&[0]).await.unwrap();
            }
            _ => peer.drops.closed.store(true, Ordering::SeqCst),
        }
        released(&peer.drops).await;
        dispatcher.dispatch(1, 7, ip, b"after-flush", false).await.unwrap();
        let mut replacement = h.next_peer().await;
        assert_eq!(read_packet(&mut replacement).await, (7, b"after-flush".to_vec()));
        assert_eq!(h.factory.attempts.load(Ordering::SeqCst), 2);
    }
}
