use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::FutureExt as _;
use rust_socketio::asynchronous::{Client, ClientBuilder};
use rust_socketio::{Payload, TransportType};
use serde_json::{Value, json};
use tokio::sync::{Mutex as AsyncMutex, Notify, mpsc};
use url::Url;

use crate::auth::StreamCredentials;
use crate::error::bounded;

use super::codec::{
    decode_candle, decode_one_click_equity, decode_one_click_fno, decode_order_notification,
    decode_tick,
};
use super::{
    CandleInterval, StreamDecodeError, StreamError, StreamEvent, StreamHandle, StreamHandleInner,
    StreamKind, Subscription, SubscriptionInsert, SubscriptionKind, SubscriptionSet,
};

const DEFAULT_EVENT_CAPACITY: usize = 256;
const DEFAULT_SUBSCRIPTION_LIMIT: usize = 2_000;
const DEFAULT_RECONNECT_ATTEMPTS: u8 = 20;
const INITIAL_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Configures authenticated Breeze Socket.IO streams.
///
/// Construct this from [`crate::BreezeClient::streaming`]. The transport is
/// WebSocket-only, reconnects with bounded backoff, and replays the deduplicated
/// desired subscription set after every authenticated reconnect.
#[derive(Clone, Debug)]
pub struct StreamingClient {
    credentials: StreamCredentials,
    live_feeds: Url,
    live_stream: Url,
    ohlcv: Url,
    event_capacity: usize,
    subscription_limit: usize,
    reconnect_min: Duration,
    reconnect_max: Duration,
    max_reconnect_attempts: u8,
}

impl StreamingClient {
    pub(crate) fn new(
        credentials: StreamCredentials,
        live_feeds: Url,
        live_stream: Url,
        ohlcv: Url,
    ) -> Self {
        Self {
            credentials,
            live_feeds,
            live_stream,
            ohlcv,
            event_capacity: DEFAULT_EVENT_CAPACITY,
            subscription_limit: DEFAULT_SUBSCRIPTION_LIMIT,
            reconnect_min: Duration::from_millis(250),
            reconnect_max: Duration::from_secs(5),
            max_reconnect_attempts: DEFAULT_RECONNECT_ATTEMPTS,
        }
    }

    /// Sets the bounded consumer queue capacity. Zero is normalized to one.
    pub fn event_capacity(mut self, value: usize) -> Self {
        self.event_capacity = value.max(1);
        self
    }

    /// Sets the local desired-subscription cap.
    pub fn subscription_limit(mut self, value: usize) -> Self {
        self.subscription_limit = value.max(1);
        self
    }

    /// Sets bounded reconnect behavior for this client.
    pub fn reconnect_policy(mut self, min: Duration, max: Duration, max_attempts: u8) -> Self {
        self.reconnect_min = min;
        self.reconnect_max = max.max(min);
        self.max_reconnect_attempts = max_attempts.max(1);
        self
    }

    /// Opens one independent stream family.
    pub async fn connect(&self, kind: StreamKind) -> Result<StreamHandle, StreamError> {
        let endpoint = match kind {
            StreamKind::MarketData => &self.live_stream,
            StreamKind::Orders | StreamKind::OneClickFno | StreamKind::OneClickEquity => {
                &self.live_feeds
            }
            StreamKind::Candles => &self.ohlcv,
        };
        let (sender, receiver) = mpsc::channel(self.event_capacity);
        let dropped = Arc::new(AtomicUsize::new(0));
        let subscriptions = Arc::new(Mutex::new(SubscriptionSet::with_limit(
            self.subscription_limit,
        )));
        let emission_guard = Arc::new(AsyncMutex::new(()));
        let generation = Arc::new(AtomicU64::new(0));
        let reconnected = Arc::new(Notify::new());
        let closed = Arc::new(AtomicBool::new(false));

        let connect_sender = sender.clone();
        let connect_dropped = dropped.clone();
        let connect_subscriptions = subscriptions.clone();
        let connect_guard = emission_guard.clone();
        let connect_generation = generation.clone();
        let connect_notify = reconnected.clone();
        let connect_closed = closed.clone();
        let mut builder = ClientBuilder::new(endpoint.as_str())
            .transport_type(TransportType::Websocket)
            .auth(json!({
                "user": self.credentials.user().expose_for_auth(),
                "token": self.credentials.token().expose_for_auth(),
            }))
            .reconnect(true)
            .reconnect_on_disconnect(true)
            .reconnect_delay(
                duration_millis(self.reconnect_min),
                duration_millis(self.reconnect_max),
            )
            .max_reconnect_attempts(self.max_reconnect_attempts)
            .opening_header("User-Agent", "breeze-icici-rust/socket.io")
            .on("open", move |_payload, socket| {
                let sender = connect_sender.clone();
                let dropped = connect_dropped.clone();
                let subscriptions = connect_subscriptions.clone();
                let guard = connect_guard.clone();
                let generation = connect_generation.clone();
                let notify = connect_notify.clone();
                let closed = connect_closed.clone();
                async move {
                    let _guard = guard.lock().await;
                    if closed.load(Ordering::SeqCst) {
                        return;
                    }
                    if let Some(room) = automatic_room(kind) {
                        if let Err(error) = socket.emit("join", json!([room])).await {
                            enqueue(&sender, &dropped, Err(connection_error(error.to_string())));
                        }
                    }
                    let active: Vec<_> = subscriptions
                        .lock()
                        .expect("subscriptions mutex poisoned")
                        .values()
                        .cloned()
                        .collect();
                    for subscription in active {
                        if let Err(error) =
                            socket.emit("join", subscription.script().to_string()).await
                        {
                            enqueue(&sender, &dropped, Err(connection_error(error.to_string())));
                        }
                    }
                    generation.fetch_add(1, Ordering::SeqCst);
                    notify.notify_waiters();
                }
                .boxed()
            });

        builder = register_data_callbacks(
            builder,
            kind,
            sender.clone(),
            dropped.clone(),
            subscriptions.clone(),
        );

        let error_sender = sender.clone();
        let error_dropped = dropped.clone();
        builder = builder.on("error", move |_payload, _socket| {
            let sender = error_sender.clone();
            let dropped = error_dropped.clone();
            async move {
                enqueue(
                    &sender,
                    &dropped,
                    Err(StreamError::Connection {
                        message: "Socket.IO reported a connection or protocol error".to_owned(),
                    }),
                );
            }
            .boxed()
        });

        let socket = builder
            .connect()
            .await
            .map_err(|error| connection_error(error.to_string()))?;
        if tokio::time::timeout(
            INITIAL_CONNECT_TIMEOUT,
            wait_for_generation(&generation, &reconnected, &closed, 0),
        )
        .await
        .is_err()
        {
            closed.store(true, Ordering::SeqCst);
            let _ = socket.disconnect().await;
            return Err(StreamError::Connection {
                message: "Socket.IO authentication handshake timed out".to_owned(),
            });
        }
        let observed_generation = generation.load(Ordering::SeqCst);

        Ok(StreamHandle {
            inner: StreamHandleInner::Production(ProductionStreamHandle {
                socket,
                kind,
                receiver,
                dropped,
                subscriptions,
                emission_guard,
                generation,
                reconnected,
                observed_generation,
                closed,
            }),
        })
    }
}

pub(crate) struct ProductionStreamHandle {
    socket: Client,
    kind: StreamKind,
    receiver: mpsc::Receiver<Result<StreamEvent, StreamError>>,
    dropped: Arc<AtomicUsize>,
    subscriptions: Arc<Mutex<SubscriptionSet>>,
    emission_guard: Arc<AsyncMutex<()>>,
    generation: Arc<AtomicU64>,
    reconnected: Arc<Notify>,
    observed_generation: u64,
    closed: Arc<AtomicBool>,
}

impl std::fmt::Debug for ProductionStreamHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProductionStreamHandle")
            .field("kind", &self.kind)
            .field("closed", &self.closed.load(Ordering::SeqCst))
            .finish_non_exhaustive()
    }
}

impl Drop for ProductionStreamHandle {
    fn drop(&mut self) {
        if self.closed.swap(true, Ordering::SeqCst) {
            return;
        }
        self.reconnected.notify_waiters();
        let socket = self.socket.clone();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            drop(runtime.spawn(async move {
                let _ = socket.disconnect().await;
            }));
        }
    }
}

impl ProductionStreamHandle {
    pub async fn subscribe(
        &mut self,
        value: Subscription,
    ) -> Result<SubscriptionInsert, StreamError> {
        self.ensure_open()?;
        validate_subscription(self.kind, &value)?;
        let _guard = self.emission_guard.lock().await;
        self.ensure_open()?;
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
        if result == SubscriptionInsert::AlreadyPresent {
            return Ok(result);
        }
        if let Err(error) = self.socket.emit("join", value.script().to_string()).await {
            self.subscriptions
                .lock()
                .expect("subscriptions mutex poisoned")
                .remove(&value);
            return Err(connection_error(error.to_string()));
        }
        Ok(result)
    }

    pub async fn unsubscribe(&mut self, value: &Subscription) -> Result<bool, StreamError> {
        self.ensure_open()?;
        validate_subscription(self.kind, value)?;
        let _guard = self.emission_guard.lock().await;
        self.ensure_open()?;
        let removed = self
            .subscriptions
            .lock()
            .expect("subscriptions mutex poisoned")
            .remove(value);
        if !removed {
            return Ok(false);
        }
        if let Err(error) = self.socket.emit("leave", value.script().to_string()).await {
            let _ = self
                .subscriptions
                .lock()
                .expect("subscriptions mutex poisoned")
                .insert(value.clone());
            return Err(connection_error(error.to_string()));
        }
        Ok(true)
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

    pub async fn wait_until_reconnected(&mut self) -> Result<(), StreamError> {
        self.ensure_open()?;
        wait_for_generation(
            &self.generation,
            &self.reconnected,
            &self.closed,
            self.observed_generation,
        )
        .await?;
        self.observed_generation = self.generation.load(Ordering::SeqCst);
        Ok(())
    }

    pub async fn shutdown(&mut self) -> Result<(), StreamError> {
        if self.closed.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        let _guard = self.emission_guard.lock().await;
        let active: Vec<_> = self
            .subscriptions
            .lock()
            .expect("subscriptions mutex poisoned")
            .values()
            .cloned()
            .collect();
        let mut first_error = None;
        for subscription in active {
            if let Err(error) = self
                .socket
                .emit("leave", subscription.script().to_string())
                .await
            {
                first_error.get_or_insert_with(|| connection_error(error.to_string()));
            }
        }
        if let Some(room) = automatic_room(self.kind) {
            if let Err(error) = self.socket.emit("leave", json!([room])).await {
                first_error.get_or_insert_with(|| connection_error(error.to_string()));
            }
        }
        if let Err(error) = self.socket.disconnect().await {
            first_error.get_or_insert_with(|| connection_error(error.to_string()));
        }
        self.reconnected.notify_waiters();
        if let Some(error) = first_error {
            Err(error)
        } else {
            Ok(())
        }
    }

    fn ensure_open(&self) -> Result<(), StreamError> {
        if self.closed.load(Ordering::SeqCst) {
            Err(StreamError::Closed)
        } else {
            Ok(())
        }
    }
}

fn register_data_callbacks(
    mut builder: ClientBuilder,
    kind: StreamKind,
    sender: mpsc::Sender<Result<StreamEvent, StreamError>>,
    dropped: Arc<AtomicUsize>,
    subscriptions: Arc<Mutex<SubscriptionSet>>,
) -> ClientBuilder {
    match kind {
        StreamKind::MarketData | StreamKind::OneClickFno | StreamKind::OneClickEquity => {
            builder = builder.on("stock", data_callback(kind, None, sender, dropped, None));
        }
        StreamKind::Orders => {
            builder = builder.on("order", data_callback(kind, None, sender, dropped, None));
        }
        StreamKind::Candles => {
            for interval in [
                CandleInterval::OneSecond,
                CandleInterval::OneMinute,
                CandleInterval::FiveMinutes,
                CandleInterval::ThirtyMinutes,
            ] {
                builder = builder.on(
                    interval.channel(),
                    data_callback(
                        kind,
                        Some(interval),
                        sender.clone(),
                        dropped.clone(),
                        Some(subscriptions.clone()),
                    ),
                );
            }
        }
    }
    builder
}

fn data_callback(
    kind: StreamKind,
    interval: Option<CandleInterval>,
    sender: mpsc::Sender<Result<StreamEvent, StreamError>>,
    dropped: Arc<AtomicUsize>,
    subscriptions: Option<Arc<Mutex<SubscriptionSet>>>,
) -> impl FnMut(Payload, Client) -> futures_util::future::BoxFuture<'static, ()> + Send + Sync + 'static
{
    move |payload, _socket| {
        let sender = sender.clone();
        let dropped = dropped.clone();
        let subscriptions = subscriptions.clone();
        async move {
            if let (Some(interval), Some(subscriptions)) = (interval, subscriptions) {
                let wanted = subscriptions
                    .lock()
                    .expect("subscriptions mutex poisoned")
                    .values()
                    .any(|subscription| subscription.kind() == &SubscriptionKind::Candle(interval));
                if !wanted {
                    return;
                }
            }
            let event = decode_payload(kind, interval, payload).map_err(StreamError::Decode);
            enqueue(&sender, &dropped, event);
        }
        .boxed()
    }
}

fn decode_payload(
    kind: StreamKind,
    expected_interval: Option<CandleInterval>,
    payload: Payload,
) -> Result<StreamEvent, StreamDecodeError> {
    let value = normalize_payload(payload)?;
    match kind {
        StreamKind::MarketData => decode_tick(&value),
        StreamKind::Orders => decode_order_notification(&value).map(StreamEvent::Order),
        StreamKind::OneClickFno => decode_one_click_fno(&value).map(StreamEvent::OneClickFno),
        StreamKind::OneClickEquity => {
            decode_one_click_equity(&value).map(StreamEvent::OneClickEquity)
        }
        StreamKind::Candles => {
            let text = value
                .as_str()
                .ok_or_else(|| StreamDecodeError::new("candle frame must be CSV text"))?;
            let event = decode_candle(text)?;
            if let (Some(expected), StreamEvent::Candle(candle)) = (expected_interval, &event) {
                if candle.interval() != expected {
                    return Err(StreamDecodeError::new(
                        "candle event channel and payload interval disagree",
                    ));
                }
            }
            Ok(event)
        }
    }
}

fn normalize_payload(payload: Payload) -> Result<Value, StreamDecodeError> {
    match payload {
        Payload::Text(mut values) => match values.len() {
            0 => Err(StreamDecodeError::new("Socket.IO event payload is empty")),
            1 => Ok(values.remove(0)),
            _ => Ok(Value::Array(values)),
        },
        Payload::Binary(_) => Err(StreamDecodeError::new(
            "binary Socket.IO payloads are not supported by Breeze streams",
        )),
        #[allow(deprecated)]
        Payload::String(value) => serde_json::from_str(&value).or(Ok(Value::String(value))),
    }
}

fn validate_subscription(kind: StreamKind, value: &Subscription) -> Result<(), StreamError> {
    if value.is_valid_for(kind) {
        Ok(())
    } else {
        Err(StreamError::InvalidSubscription)
    }
}

fn automatic_room(kind: StreamKind) -> Option<&'static str> {
    match kind {
        StreamKind::OneClickFno => Some("one_click_fno"),
        StreamKind::OneClickEquity => Some("i_click_2_gain"),
        _ => None,
    }
}

fn enqueue(
    sender: &mpsc::Sender<Result<StreamEvent, StreamError>>,
    dropped: &AtomicUsize,
    event: Result<StreamEvent, StreamError>,
) {
    if sender.try_send(event).is_err() {
        dropped.fetch_add(1, Ordering::SeqCst);
    }
}

async fn wait_for_generation(
    generation: &AtomicU64,
    notify: &Notify,
    closed: &AtomicBool,
    observed: u64,
) -> Result<(), StreamError> {
    loop {
        let notified = notify.notified();
        if generation.load(Ordering::SeqCst) > observed {
            return Ok(());
        }
        if closed.load(Ordering::SeqCst) {
            return Err(StreamError::Closed);
        }
        notified.await;
    }
}

fn duration_millis(value: Duration) -> u64 {
    value.as_millis().min(u64::MAX as u128) as u64
}

fn connection_error(message: String) -> StreamError {
    StreamError::Connection {
        message: bounded(message),
    }
}
