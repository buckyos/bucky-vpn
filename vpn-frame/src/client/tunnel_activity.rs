use crate::server::NodeId;
use crate::{VpnTunnelRecv, VpnTunnelSend};
use std::io::{Error, ErrorKind};
use std::pin::Pin;
use std::sync::{Arc, Mutex, Weak};
use std::task::{Context, Poll, Waker};
use std::time::Duration;
use tokio::io::{AsyncRead, ReadBuf};
use tokio::sync::Notify;
use tokio::task::AbortHandle;
use tokio::time::Instant;

struct ChannelState<S> {
    send: Option<S>,
    recv_abort: Option<AbortHandle>,
    last_activity: Instant,
    sending: bool,
    pending_writer: Option<Waker>,
}

pub(super) struct TunnelChannel<S: VpnTunnelSend> {
    state: Mutex<ChannelState<S>>,
    closed: Notify,
}

pub(super) struct SendPermit<S: VpnTunnelSend> {
    channel: Arc<TunnelChannel<S>>,
}

impl<S: VpnTunnelSend> Drop for SendPermit<S> {
    fn drop(&mut self) {
        let pending_writer = {
            let mut state = self.channel.state.lock().unwrap();
            state.sending = false;
            state.pending_writer.take()
        };
        drop(pending_writer);
    }
}

impl<S: VpnTunnelSend> TunnelChannel<S> {
    pub fn new(send: S) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(ChannelState {
                send: Some(send),
                recv_abort: None,
                last_activity: Instant::now(),
                sending: false,
                pending_writer: None,
            }),
            closed: Notify::new(),
        })
    }

    pub fn begin_send(self: &Arc<Self>) -> Option<SendPermit<S>> {
        let mut state = self.state.lock().unwrap();
        if state.sending || state.send.as_ref().is_none_or(|send| send.is_closed()) {
            return None;
        }
        state.sending = true;
        Some(SendPermit {
            channel: self.clone(),
        })
    }

    pub fn is_work(&self) -> bool {
        self.state
            .lock()
            .unwrap()
            .send
            .as_ref()
            .is_some_and(|send| !send.is_closed())
    }

    pub fn is_target_tunnel(&self, target: &NodeId) -> bool {
        self.state
            .lock()
            .unwrap()
            .send
            .as_ref()
            .is_some_and(|send| send.is_target_tunnel(target))
    }

    pub fn set_recv_abort(&self, handle: AbortHandle) {
        let mut state = self.state.lock().unwrap();
        if state.send.is_some() {
            state.recv_abort = Some(handle);
        } else {
            handle.abort();
        }
    }

    fn take_closed(
        state: &mut ChannelState<S>,
    ) -> (Option<S>, Option<AbortHandle>, Option<Waker>) {
        state.sending = false;
        (
            state.send.take(),
            state.recv_abort.take(),
            state.pending_writer.take(),
        )
    }

    fn finish_close(&self, resources: (Option<S>, Option<AbortHandle>, Option<Waker>)) {
        let (send, recv_abort, pending_writer) = resources;
        if let Some(handle) = recv_abort {
            handle.abort();
        }
        drop(send);
        self.closed.notify_waiters();
        if let Some(waker) = pending_writer {
            waker.wake();
        }
    }

    pub fn expire_idle(&self, now: Instant, timeout: Duration) {
        let resources = {
            let mut state = self.state.lock().unwrap();
            let transport_closed = state.send.as_ref().is_some_and(|send| send.is_closed());
            if !transport_closed
                && (state.sending || now.saturating_duration_since(state.last_activity) < timeout)
            {
                return;
            }
            Self::take_closed(&mut state)
        };
        self.finish_close(resources);
    }

    pub fn close(&self) {
        let resources = Self::take_closed(&mut self.state.lock().unwrap());
        self.finish_close(resources);
    }

    pub async fn closed(&self) {
        loop {
            let notified = self.closed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.state.lock().unwrap().send.is_none() {
                return;
            }
            notified.await;
        }
    }

    pub fn poll_write(&self, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize, Error>> {
        let waker = cx.waker().clone();
        let (result, previous_writer) = {
            let mut state = self.state.lock().unwrap();
            let Some(send) = state.send.as_mut() else {
                return Poll::Ready(Err(ErrorKind::BrokenPipe.into()));
            };
            let result = Pin::new(send).poll_write(cx, buf);
            let previous_writer = if result.is_pending() {
                state.pending_writer.replace(waker)
            } else {
                state.pending_writer.take()
            };
            if matches!(&result, Poll::Ready(Ok(n)) if *n > 0) {
                state.last_activity = Instant::now();
            }
            (result, previous_writer)
        };
        // Waker callbacks may reenter the channel, including when replaced or dropped.
        drop(previous_writer);
        if matches!(&result, Poll::Ready(Err(_)))
            || matches!(&result, Poll::Ready(Ok(0)) if !buf.is_empty())
        {
            self.close();
        }
        result
    }

    pub fn poll_flush(&self, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        let waker = cx.waker().clone();
        let (result, previous_writer) = {
            let mut state = self.state.lock().unwrap();
            let Some(send) = state.send.as_mut() else {
                return Poll::Ready(Err(ErrorKind::BrokenPipe.into()));
            };
            let result = Pin::new(send).poll_flush(cx);
            let previous_writer = if result.is_pending() {
                state.pending_writer.replace(waker)
            } else {
                state.pending_writer.take()
            };
            (result, previous_writer)
        };
        drop(previous_writer);
        if matches!(&result, Poll::Ready(Err(_))) {
            self.close();
        }
        result
    }

    pub fn poll_shutdown(&self, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        let waker = cx.waker().clone();
        let (result, previous_writer) = {
            let mut state = self.state.lock().unwrap();
            let Some(send) = state.send.as_mut() else {
                return Poll::Ready(Err(ErrorKind::BrokenPipe.into()));
            };
            let result = Pin::new(send).poll_shutdown(cx);
            let previous_writer = if result.is_pending() {
                state.pending_writer.replace(waker)
            } else {
                state.pending_writer.take()
            };
            (result, previous_writer)
        };
        drop(previous_writer);
        if result.is_ready() {
            self.close();
        }
        result
    }
}

pub(super) struct TrackedRecv<R: VpnTunnelRecv, S: VpnTunnelSend> {
    recv: R,
    channel: Weak<TunnelChannel<S>>,
}

impl<R: VpnTunnelRecv, S: VpnTunnelSend> TrackedRecv<R, S> {
    pub fn new(recv: R, channel: &Arc<TunnelChannel<S>>) -> Self {
        Self {
            recv,
            channel: Arc::downgrade(channel),
        }
    }
}

impl<R: VpnTunnelRecv, S: VpnTunnelSend> AsyncRead for TrackedRecv<R, S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<(), Error>> {
        let Some(channel) = self.channel.upgrade() else {
            return Poll::Ready(Err(ErrorKind::BrokenPipe.into()));
        };
        let mut state = channel.state.lock().unwrap();
        if state.send.is_none() {
            return Poll::Ready(Err(ErrorKind::BrokenPipe.into()));
        }
        let before = buf.filled().len();
        let result = Pin::new(&mut self.recv).poll_read(cx, buf);
        if buf.filled().len() > before {
            state.last_activity = Instant::now();
        }
        result
    }
}

struct RegistryState<S: VpnTunnelSend> {
    channels: Vec<Weak<TunnelChannel<S>>>,
    closed: bool,
}

pub(super) struct ChannelRegistry<S: VpnTunnelSend> {
    state: Mutex<RegistryState<S>>,
}

impl<S: VpnTunnelSend> ChannelRegistry<S> {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(RegistryState {
                channels: Vec::new(),
                closed: false,
            }),
        }
    }

    pub fn register(&self, channel: &Arc<TunnelChannel<S>>) {
        let mut state = self.state.lock().unwrap();
        if state.closed {
            drop(state);
            channel.close();
        } else {
            state.channels.push(Arc::downgrade(channel));
        }
    }

    pub fn cleanup(&self, now: Instant, timeout: Duration) {
        let channels = self.state.lock().unwrap().channels.clone();
        for channel in channels.into_iter().filter_map(|channel| channel.upgrade()) {
            channel.expire_idle(now, timeout);
        }
        self.state.lock().unwrap().channels.retain(|channel| {
            channel
                .upgrade()
                .is_some_and(|channel| channel.state.lock().unwrap().send.is_some())
        });
    }

    pub fn close_all(&self) {
        let channels = {
            let mut state = self.state.lock().unwrap();
            state.closed = true;
            std::mem::take(&mut state.channels)
        };
        for channel in channels.into_iter().filter_map(|channel| channel.upgrade()) {
            channel.close();
        }
    }
}
