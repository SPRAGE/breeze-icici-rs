use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;
use tokio::sync::{Notify, mpsc};

use crate::auth::StreamCredentials;

use super::codec::{
    decode_one_click_equity, decode_one_click_fno, decode_order_notification, decode_tick,
};
use super::{
    StreamError, StreamEvent, StreamHandle, StreamHandleInner, StreamKind, Subscription,
    SubscriptionInsert, SubscriptionSet,
};

#[derive(Clone, Debug)]
pub struct FakeSocketIo {
    state: Arc<FakeState>,
}

#[derive(Debug)]
struct FakeState {
    inner: Mutex<FakeInner>,
    reconnect: Notify,
}

#[derive(Debug, Default)]
struct FakeInner {
    connections: Vec<ConnectionRecord>,
    streams: Vec<StreamRegistration>,
}

#[derive(Debug, Default)]
struct ConnectionRecord {
    auth_count: usize,
    joins: HashMap<Subscription, usize>,
    leaves: HashMap<Subscription, usize>,
}

#[derive(Debug)]
struct StreamRegistration {
    kind: StreamKind,
    sender: mpsc::Sender<Result<StreamEvent, StreamError>>,
    dropped: Arc<AtomicUsize>,
    subscriptions: Arc<Mutex<SubscriptionSet>>,
    shutdown: Arc<AtomicBool>,
}

impl FakeSocketIo {
    pub fn new() -> Self {
        Self {
            state: Arc::new(FakeState {
                inner: Mutex::new(FakeInner::default()),
                reconnect: Notify::new(),
            }),
        }
    }
    pub fn transport(&self) -> FakeSocketTransport {
        FakeSocketTransport {
            state: self.state.clone(),
        }
    }

    pub async fn disconnect(&self) {}

    pub async fn allow_next_connection(&self) {
        let mut inner = self.state.inner.lock().expect("fake socket mutex poisoned");
        let mut connection = ConnectionRecord {
            auth_count: 1,
            ..ConnectionRecord::default()
        };
        for stream in &inner.streams {
            if stream.shutdown.load(Ordering::SeqCst) {
                continue;
            }
            for subscription in stream
                .subscriptions
                .lock()
                .expect("subscriptions mutex poisoned")
                .values()
            {
                *connection.joins.entry(subscription.clone()).or_default() += 1;
            }
        }
        inner.connections.push(connection);
        drop(inner);
        self.state.reconnect.notify_waiters();
    }

    pub async fn connection(&self, index: usize) -> FakeConnection {
        loop {
            if self
                .state
                .inner
                .lock()
                .expect("fake socket mutex poisoned")
                .connections
                .len()
                > index
            {
                return FakeConnection {
                    state: self.state.clone(),
                    index,
                };
            }
            self.state.reconnect.notified().await;
        }
    }

    pub async fn connection_count(&self) -> usize {
        self.state
            .inner
            .lock()
            .expect("fake socket mutex poisoned")
            .connections
            .len()
    }

    pub async fn emit(&self, event: &str, value: Value) {
        let streams: Vec<_> = {
            let inner = self.state.inner.lock().expect("fake socket mutex poisoned");
            inner
                .streams
                .iter()
                .filter(|stream| !stream.shutdown.load(Ordering::SeqCst))
                .map(|stream| (stream.kind, stream.sender.clone(), stream.dropped.clone()))
                .collect()
        };
        for (kind, sender, dropped) in streams {
            let decoded = match (kind, event) {
                (StreamKind::Orders, "order") => {
                    decode_order_notification(&value).map(StreamEvent::Order)
                }
                (StreamKind::MarketData, "stock") => decode_tick(&value),
                (StreamKind::OneClickFno, "stock") => {
                    decode_one_click_fno(&value).map(StreamEvent::OneClickFno)
                }
                (StreamKind::OneClickEquity, "stock") => {
                    decode_one_click_equity(&value).map(StreamEvent::OneClickEquity)
                }
                _ => continue,
            }
            .map_err(StreamError::Decode);
            if sender.try_send(decoded).is_err() {
                dropped.fetch_add(1, Ordering::SeqCst);
            }
        }
    }
}

impl Default for FakeSocketIo {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub struct FakeSocketTransport {
    state: Arc<FakeState>,
}

#[derive(Clone, Debug)]
pub struct FakeConnection {
    state: Arc<FakeState>,
    index: usize,
}
impl FakeConnection {
    pub fn auth_count(&self) -> usize {
        self.with(|value| value.auth_count)
    }
    pub fn join_count(&self, subscription: &Subscription) -> usize {
        self.with(|value| value.joins.get(subscription).copied().unwrap_or(0))
    }
    pub fn total_join_count(&self) -> usize {
        self.with(|value| value.joins.values().sum())
    }
    pub fn leave_count(&self, subscription: &Subscription) -> usize {
        self.with(|value| value.leaves.get(subscription).copied().unwrap_or(0))
    }
    fn with<T>(&self, operation: impl FnOnce(&ConnectionRecord) -> T) -> T {
        let inner = self.state.inner.lock().expect("fake socket mutex poisoned");
        operation(&inner.connections[self.index])
    }
}

#[derive(Debug)]
pub struct StreamTestClientBuilder {
    credentials: StreamCredentials,
    transport: Option<FakeSocketTransport>,
    event_capacity: usize,
    reconnect_min: Duration,
    reconnect_max: Duration,
}

#[derive(Clone, Debug)]
pub struct StreamTestClient {
    transport: FakeSocketTransport,
    event_capacity: usize,
}

impl StreamTestClient {
    pub fn builder(credentials: StreamCredentials) -> StreamTestClientBuilder {
        StreamTestClientBuilder {
            credentials,
            transport: None,
            event_capacity: 256,
            reconnect_min: Duration::from_millis(100),
            reconnect_max: Duration::from_secs(5),
        }
    }

    pub async fn connect(&self, kind: StreamKind) -> Result<StreamHandle, StreamError> {
        let (sender, receiver) = mpsc::channel(self.event_capacity);
        let dropped = Arc::new(AtomicUsize::new(0));
        let subscriptions = Arc::new(Mutex::new(SubscriptionSet::with_limit(2_000)));
        let shutdown = Arc::new(AtomicBool::new(false));
        let connection_index = {
            let mut inner = self
                .transport
                .state
                .inner
                .lock()
                .expect("fake socket mutex poisoned");
            let index = inner.connections.len();
            inner.connections.push(ConnectionRecord {
                auth_count: 1,
                ..ConnectionRecord::default()
            });
            inner.streams.push(StreamRegistration {
                kind,
                sender,
                dropped: dropped.clone(),
                subscriptions: subscriptions.clone(),
                shutdown: shutdown.clone(),
            });
            index
        };
        Ok(StreamHandle {
            inner: StreamHandleInner::Test(TestStreamHandle {
                state: self.transport.state.clone(),
                kind,
                receiver,
                dropped,
                subscriptions,
                shutdown,
                connection_index,
            }),
        })
    }
}

impl StreamTestClientBuilder {
    pub fn transport(mut self, value: FakeSocketTransport) -> Self {
        self.transport = Some(value);
        self
    }
    pub fn event_capacity(mut self, value: usize) -> Self {
        self.event_capacity = value.max(1);
        self
    }
    pub fn reconnect_backoff(mut self, min: Duration, max: Duration) -> Self {
        self.reconnect_min = min;
        self.reconnect_max = max.max(min);
        self
    }
    pub fn build(self) -> StreamTestClient {
        let _ = self.credentials;
        StreamTestClient {
            transport: self.transport.expect("test stream transport is required"),
            event_capacity: self.event_capacity,
        }
    }
}

pub struct TestStreamHandle {
    state: Arc<FakeState>,
    kind: StreamKind,
    receiver: mpsc::Receiver<Result<StreamEvent, StreamError>>,
    dropped: Arc<AtomicUsize>,
    subscriptions: Arc<Mutex<SubscriptionSet>>,
    shutdown: Arc<AtomicBool>,
    connection_index: usize,
}

impl fmt::Debug for TestStreamHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TestStreamHandle")
            .finish_non_exhaustive()
    }
}

use std::fmt;

impl TestStreamHandle {
    pub async fn subscribe(
        &mut self,
        value: Subscription,
    ) -> Result<SubscriptionInsert, StreamError> {
        if self.shutdown.load(Ordering::SeqCst) {
            return Err(StreamError::Closed);
        }
        let result = {
            let mut subscriptions = self
                .subscriptions
                .lock()
                .expect("subscriptions mutex poisoned");
            if !subscriptions.accepts(self.kind, &value) {
                return Err(StreamError::InvalidSubscription);
            }
            subscriptions.insert(value.clone())?
        };
        if result == SubscriptionInsert::Added {
            let mut inner = self.state.inner.lock().expect("fake socket mutex poisoned");
            *inner.connections[self.connection_index]
                .joins
                .entry(value)
                .or_default() += 1;
        }
        Ok(result)
    }

    pub async fn next_event(&mut self) -> Option<Result<StreamEvent, StreamError>> {
        match self.receiver.try_recv() {
            Ok(event) => return Some(event),
            Err(mpsc::error::TryRecvError::Disconnected) => return None,
            Err(mpsc::error::TryRecvError::Empty) => {}
        }
        let dropped = self.dropped.swap(0, Ordering::SeqCst);
        if dropped > 0 {
            Some(Err(StreamError::LaggedRequiresReconciliation { dropped }))
        } else {
            self.receiver.recv().await
        }
    }

    pub async fn unsubscribe(&mut self, value: &Subscription) -> Result<bool, StreamError> {
        if self.shutdown.load(Ordering::SeqCst) {
            return Err(StreamError::Closed);
        }
        if !value.is_valid_for(self.kind) {
            return Err(StreamError::InvalidSubscription);
        }
        let removed = self
            .subscriptions
            .lock()
            .expect("subscriptions mutex poisoned")
            .remove(value);
        if removed {
            let mut inner = self.state.inner.lock().expect("fake socket mutex poisoned");
            *inner.connections[self.connection_index]
                .leaves
                .entry(value.clone())
                .or_default() += 1;
        }
        Ok(removed)
    }

    pub async fn wait_until_reconnected(&mut self) -> Result<(), StreamError> {
        loop {
            let count = self
                .state
                .inner
                .lock()
                .expect("fake socket mutex poisoned")
                .connections
                .len();
            if count > self.connection_index + 1 {
                self.connection_index = count - 1;
                return Ok(());
            }
            self.state.reconnect.notified().await;
        }
    }

    pub async fn shutdown(&mut self) -> Result<(), StreamError> {
        if self.shutdown.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        let subscriptions: Vec<_> = self
            .subscriptions
            .lock()
            .expect("subscriptions mutex poisoned")
            .values()
            .cloned()
            .collect();
        let mut inner = self.state.inner.lock().expect("fake socket mutex poisoned");
        for subscription in subscriptions {
            *inner.connections[self.connection_index]
                .leaves
                .entry(subscription)
                .or_default() += 1;
        }
        Ok(())
    }
}
