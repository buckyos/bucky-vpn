use super::{ChannelRegistry, TrackedRecv, TunnelChannel};
use crate::{VpnTunnelRecv, VpnTunnelSend};
use crate::server::NodeId;
use std::io::{Error, ErrorKind};
use std::pin::Pin;
use std::sync::{Arc, atomic::{AtomicBool, AtomicUsize, Ordering}};
use std::task::{Context, Poll, Wake, Waker};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::time::Instant;

const IDLE: Duration = Duration::from_secs(600);

#[derive(Default)]
struct Probe {
    drops: AtomicUsize,
    mode: AtomicUsize,
    closed: AtomicBool,
}

struct Send(Arc<Probe>);
impl Drop for Send {
    fn drop(&mut self) { self.0.drops.fetch_add(1, Ordering::SeqCst); }
}
impl VpnTunnelSend for Send {
    fn is_target_tunnel(&self, _: &NodeId) -> bool { true }
    fn is_closed(&self) -> bool { self.0.closed.load(Ordering::SeqCst) }
}
impl AsyncWrite for Send {
    fn poll_write(self: Pin<&mut Self>, _: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize, Error>> {
        match self.0.mode.load(Ordering::SeqCst) {
            1 => Poll::Pending,
            2 => Poll::Ready(Err(ErrorKind::ConnectionReset.into())),
            3 => Poll::Ready(Ok(0)),
            _ => Poll::Ready(Ok(buf.len().min(2))),
        }
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Error>> {
        match self.0.mode.load(Ordering::SeqCst) {
            1 => Poll::Pending,
            2 => Poll::Ready(Err(ErrorKind::BrokenPipe.into())),
            _ => Poll::Ready(Ok(())),
        }
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        self.poll_flush(cx)
    }
}
struct Recv { mode: usize }
impl VpnTunnelRecv for Recv {}
impl AsyncRead for Recv {
    fn poll_read(self: Pin<&mut Self>, _: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<Result<(), Error>> {
        match self.mode {
            0 => Poll::Pending,
            1 => { if buf.remaining() > 0 { buf.put_slice(&[42]); } Poll::Ready(Ok(())) }
            _ => Poll::Ready(Err(ErrorKind::ConnectionReset.into())),
        }
    }
}
fn channel() -> (Arc<TunnelChannel<Send>>, Arc<Probe>) {
    let probe = Arc::new(Probe::default());
    (TunnelChannel::new(Send(probe.clone())), probe)
}
fn cx() -> Context<'static> { Context::from_waker(Waker::noop()) }

#[test]
fn exact_deadline_closes_once_and_rejects_reentry() {
    let (channel, probe) = channel();
    let created = channel.state.lock().unwrap().last_activity;
    channel.expire_idle(created + IDLE - Duration::from_nanos(1), IDLE);
    assert!(channel.is_work());
    channel.expire_idle(created + IDLE, IDLE);
    assert!(!channel.is_work());
    assert!(channel.begin_send().is_none());
    channel.close();
    assert_eq!(probe.drops.load(Ordering::SeqCst), 1);
    assert!(matches!(channel.poll_write(&mut cx(), &[1]), Poll::Ready(Err(_))));
}

#[test]
fn reservation_is_exclusive_and_survives_pending_write_and_flush() {
    let (channel, probe) = channel();
    probe.mode.store(1, Ordering::SeqCst);
    let permit = channel.begin_send().unwrap();
    assert!(channel.begin_send().is_none());
    assert!(channel.poll_write(&mut cx(), &[1]).is_pending());
    assert!(channel.poll_flush(&mut cx()).is_pending());
    channel.expire_idle(Instant::now() + IDLE * 2, IDLE);
    assert_eq!(probe.drops.load(Ordering::SeqCst), 0);
    probe.mode.store(0, Ordering::SeqCst);
    assert!(matches!(channel.poll_flush(&mut cx()), Poll::Ready(Ok(()))));
    channel.expire_idle(Instant::now() + IDLE * 2, IDLE);
    assert!(channel.is_work(), "permit owns the whole batch, including after flush");
    drop(permit);
    channel.expire_idle(Instant::now() + IDLE * 2, IDLE);
    assert_eq!(probe.drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn cancellation_drops_reservation() {
    let (channel, probe) = channel();
    let held = channel.clone();
    let (tx, rx) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let _permit = held.begin_send().unwrap();
        tx.send(()).unwrap();
        std::future::pending::<()>().await;
    });
    rx.await.unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    channel.expire_idle(Instant::now() + IDLE, IDLE);
    assert_eq!(probe.drops.load(Ordering::SeqCst), 1);
}

#[test]
fn positive_partial_send_and_receive_refresh_activity() {
    for receive in [false, true] {
        let (channel, _) = channel();
        let stale = Instant::now() - Duration::from_secs(1);
        channel.state.lock().unwrap().last_activity = stale;
        if receive {
            let mut recv = TrackedRecv::new(Recv { mode: 1 }, &channel);
            let mut bytes = [0; 2];
            let mut buf = ReadBuf::new(&mut bytes);
            assert!(matches!(Pin::new(&mut recv).poll_read(&mut cx(), &mut buf), Poll::Ready(Ok(()))));
            assert_eq!(buf.filled(), &[42]);
        } else {
            assert!(matches!(channel.poll_write(&mut cx(), &[1, 2, 3]), Poll::Ready(Ok(2))));
        }
        channel.expire_idle(stale + IDLE, IDLE);
        assert!(channel.is_work());
        let refreshed = channel.state.lock().unwrap().last_activity;
        assert!(refreshed > stale);
        channel.expire_idle(refreshed + IDLE, IDLE);
        assert!(!channel.is_work());
    }
}

#[test]
fn empty_io_pending_read_and_flush_do_not_refresh_activity() {
    for mode in [0, 1] {
        let (channel, _) = channel();
        let after_creation = channel.state.lock().unwrap().last_activity;
        let mut recv = TrackedRecv::new(Recv { mode }, &channel);
        let mut bytes = [];
        let _ = Pin::new(&mut recv).poll_read(&mut cx(), &mut ReadBuf::new(&mut bytes));
        assert!(matches!(channel.poll_write(&mut cx(), &[]), Poll::Ready(Ok(0))));
        assert!(matches!(channel.poll_flush(&mut cx()), Poll::Ready(Ok(()))));
        channel.expire_idle(after_creation + IDLE, IDLE);
        assert!(!channel.is_work());
    }
}

#[test]
fn write_zero_errors_flush_errors_and_shutdown_release_resources() {
    for (mode, operation) in [(2, 0), (3, 0), (2, 1), (0, 2), (2, 2)] {
        let (channel, probe) = channel();
        probe.mode.store(mode, Ordering::SeqCst);
        let _permit = channel.begin_send().unwrap();
        match operation {
            0 => { let _ = channel.poll_write(&mut cx(), &[1]); }
            1 => { let _ = channel.poll_flush(&mut cx()); }
            _ => { let _ = channel.poll_shutdown(&mut cx()); }
        }
        assert!(!channel.is_work());
        assert_eq!(probe.drops.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn closed_waiters_and_late_abort_are_race_safe() {
    let (channel, _) = channel();
    let mut tasks = Vec::new();
    for _ in 0..8 {
        let channel = channel.clone();
        tasks.push(tokio::spawn(async move { channel.closed().await }));
    }
    tokio::task::yield_now().await;
    channel.close();
    for task in tasks { tokio::time::timeout(Duration::from_secs(1), task).await.unwrap().unwrap(); }
    tokio::time::timeout(Duration::from_secs(1), channel.closed()).await.unwrap();
    let task = tokio::spawn(std::future::pending::<()>());
    channel.set_recv_abort(task.abort_handle());
    assert!(task.await.unwrap_err().is_cancelled());
}

#[test]
fn registry_handles_dead_transports_weak_owners_and_shutdown_registration() {
    let registry = ChannelRegistry::new();
    let (first, first_probe) = channel();
    let (second, second_probe) = channel();
    registry.register(&first);
    registry.register(&second);
    first_probe.closed.store(true, Ordering::SeqCst);
    let _permit = first.begin_send();
    registry.cleanup(Instant::now(), IDLE);
    assert_eq!(first_probe.drops.load(Ordering::SeqCst), 1);
    assert!(second.is_work());
    let (dropped, _) = channel();
    registry.register(&dropped);
    let weak = Arc::downgrade(&dropped);
    drop(dropped);
    assert!(weak.upgrade().is_none());
    registry.close_all();
    assert_eq!(second_probe.drops.load(Ordering::SeqCst), 1);
    let (late, late_probe) = channel();
    registry.register(&late);
    assert_eq!(late_probe.drops.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cleanup_and_acquisition_race_never_closes_reserved_channel() {
    for _ in 0..64 {
        let (channel, _) = channel();
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let competing = channel.clone();
        let competing_barrier = barrier.clone();
        let cleanup = tokio::task::spawn_blocking(move || {
            competing_barrier.wait();
            competing.expire_idle(Instant::now() + IDLE, IDLE);
        });
        barrier.wait();
        let permit = channel.begin_send();
        cleanup.await.unwrap();
        if let Some(permit) = permit {
            assert!(channel.is_work());
            drop(permit);
            channel.expire_idle(Instant::now() + IDLE, IDLE);
        }
        assert!(!channel.is_work());
    }
}

struct WriterWake {
    channel: std::sync::Weak<TunnelChannel<Send>>,
    wakes: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
}

impl WriterWake {
    fn assert_unlocked(&self) {
        if let Some(channel) = self.channel.upgrade() {
            assert!(channel.state.try_lock().is_ok(), "Waker callback held channel lock");
        }
    }
}

impl Wake for WriterWake {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.assert_unlocked();
        self.wakes.fetch_add(1, Ordering::SeqCst);
    }
}

impl Drop for WriterWake {
    fn drop(&mut self) {
        self.assert_unlocked();
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

fn writer_waker(channel: &Arc<TunnelChannel<Send>>) -> (Waker, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let wakes = Arc::new(AtomicUsize::new(0));
    let drops = Arc::new(AtomicUsize::new(0));
    let waker = Waker::from(Arc::new(WriterWake {
        channel: Arc::downgrade(channel),
        wakes: wakes.clone(),
        drops: drops.clone(),
    }));
    (waker, wakes, drops)
}

fn poll_writer(channel: &TunnelChannel<Send>, operation: usize, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
    match operation {
        0 => channel.poll_write(cx, &[1]).map(|result| result.map(|_| ())),
        1 => channel.poll_flush(cx),
        _ => channel.poll_shutdown(cx),
    }
}

#[test]
fn pending_writer_close_wakes_only_latest_waiter_outside_lock() {
    for operation in 0..3 {
        let (channel, probe) = channel();
        probe.mode.store(1, Ordering::SeqCst);
        let (first, first_wakes, first_drops) = writer_waker(&channel);
        assert!(poll_writer(&channel, operation, &mut Context::from_waker(&first)).is_pending());
        drop(first);
        let (second, second_wakes, second_drops) = writer_waker(&channel);
        assert!(poll_writer(&channel, operation, &mut Context::from_waker(&second)).is_pending());
        assert_eq!(first_drops.load(Ordering::SeqCst), 1, "replaced waiter retained");
        drop(second);
        channel.close();
        channel.close();
        assert_eq!(first_wakes.load(Ordering::SeqCst), 0);
        assert_eq!(second_wakes.load(Ordering::SeqCst), 1);
        assert_eq!(second_drops.load(Ordering::SeqCst), 1);
        assert!(matches!(poll_writer(&channel, operation, &mut cx()), Poll::Ready(Err(error)) if error.kind() == ErrorKind::BrokenPipe));
    }
}

#[test]
fn ready_writer_success_and_error_release_waiter_without_waking_it() {
    for operation in 0..3 {
        for mode in [0, 2] {
            let (channel, probe) = channel();
            probe.mode.store(1, Ordering::SeqCst);
            let (waker, wakes, drops) = writer_waker(&channel);
            assert!(poll_writer(&channel, operation, &mut Context::from_waker(&waker)).is_pending());
            drop(waker);
            probe.mode.store(mode, Ordering::SeqCst);
            assert!(poll_writer(&channel, operation, &mut cx()).is_ready());
            assert_eq!(drops.load(Ordering::SeqCst), 1, "completed waiter retained");
            channel.close();
            assert_eq!(wakes.load(Ordering::SeqCst), 0);
        }
    }
}

#[test]
fn cancelled_reservation_releases_pending_writer_outside_lock() {
    for operation in 0..3 {
        let (channel, probe) = channel();
        probe.mode.store(1, Ordering::SeqCst);
        let permit = channel.begin_send().unwrap();
        let (waker, wakes, drops) = writer_waker(&channel);
        assert!(poll_writer(&channel, operation, &mut Context::from_waker(&waker)).is_pending());
        drop(waker);
        drop(permit);
        assert_eq!(drops.load(Ordering::SeqCst), 1, "cancelled task retained");
        channel.expire_idle(Instant::now() + IDLE, IDLE);
        assert_eq!(wakes.load(Ordering::SeqCst), 0);
        assert!(!channel.is_work());
    }
}

#[test]
fn closed_transport_cleanup_wakes_reserved_pending_writer() {
    for operation in 0..3 {
        let registry = ChannelRegistry::new();
        let (channel, probe) = channel();
        registry.register(&channel);
        let _permit = channel.begin_send().unwrap();
        probe.mode.store(1, Ordering::SeqCst);
        let (waker, wakes, _) = writer_waker(&channel);
        assert!(poll_writer(&channel, operation, &mut Context::from_waker(&waker)).is_pending());
        probe.closed.store(true, Ordering::SeqCst);
        registry.cleanup(Instant::now(), IDLE);
        assert_eq!(wakes.load(Ordering::SeqCst), 1);
        assert!(!channel.is_work());
    }
}

#[test]
fn close_and_pending_writer_registration_race_does_not_lose_wakeup() {
    for operation in 0..3 {
        for _ in 0..32 {
            let (channel, probe) = channel();
            probe.mode.store(1, Ordering::SeqCst);
            let (waker, wakes, _) = writer_waker(&channel);
            let barrier = Arc::new(std::sync::Barrier::new(2));
            let closing = channel.clone();
            let closing_barrier = barrier.clone();
            let close = std::thread::spawn(move || {
                closing_barrier.wait();
                closing.close();
            });
            barrier.wait();
            let pending = poll_writer(&channel, operation, &mut Context::from_waker(&waker)).is_pending();
            close.join().unwrap();
            assert_eq!(wakes.load(Ordering::SeqCst), usize::from(pending));
            assert!(!channel.is_work());
        }
    }
}
