//! Per-device FIFO command queues for one WebSocket connection.
//!
//! Every device command a client sends is parked on the queue of the resource
//! it mutates and executed by that queue's worker task, in arrival order.
//! Devices therefore never wait on each other (a Kernel stuck in a reconnect
//! cannot delay Neon markers), while for a single device a `disconnect` can
//! never overtake the `send_event` that preceded it.
//!
//! Extracted from the WebSocket handler so the guarantees that matter for
//! marker integrity — per-device ordering, cross-device independence, dropping
//! still-queued commands once the connection is gone, and bounded buffering —
//! are unit-tested rather than asserted in comments.

use crate::bridge::message::CommandAction;
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, warn};

/// Pending commands allowed per device before new ones are rejected instead of
/// buffered without bound (and replayed as stale markers minutes later).
pub const DEVICE_QUEUE_CAPACITY: usize = 256;

/// Distinct queues (device keys) one connection may create. Keys come from the
/// client's `device` string; without a cap a misbehaving client could spawn a
/// worker task per arbitrary string.
pub const MAX_DEVICE_QUEUES: usize = 32;

/// A device command parked on its device's FIFO queue.
#[derive(Debug)]
pub struct QueuedCommand {
    pub device: String,
    pub action: CommandAction,
    pub payload: Option<Value>,
    pub id: Option<String>,
}

/// Executes one command. Boxed so the queue does not depend on the WebSocket
/// handler's concrete future type (and so tests can supply their own).
pub type CommandHandler =
    Arc<dyn Fn(QueuedCommand) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// Outcome of [`DeviceQueues::dispatch`].
#[derive(Debug)]
pub enum Dispatch {
    /// Accepted; will run after everything already queued for that device.
    Queued,
    /// That device's queue is full — the command is handed back so the caller
    /// can answer the client with its request id.
    Full(QueuedCommand),
    /// The connection already has [`MAX_DEVICE_QUEUES`] distinct devices.
    TooManyQueues(QueuedCommand),
    /// The worker for that device has exited (should not happen while the
    /// connection is alive).
    WorkerGone(QueuedCommand),
}

pub struct DeviceQueues {
    queues: HashMap<String, mpsc::Sender<QueuedCommand>>,
    closed: Arc<AtomicBool>,
    handler: CommandHandler,
    capacity: usize,
    max_queues: usize,
}

impl DeviceQueues {
    /// `closed` is the owning connection's flag: once it is set, workers drop
    /// every command still queued (the one in flight finishes) so a client
    /// that reconnects and retries cannot double-send markers.
    pub fn new(closed: Arc<AtomicBool>, handler: CommandHandler) -> Self {
        Self::with_limits(closed, handler, DEVICE_QUEUE_CAPACITY, MAX_DEVICE_QUEUES)
    }

    pub fn with_limits(
        closed: Arc<AtomicBool>,
        handler: CommandHandler,
        capacity: usize,
        max_queues: usize,
    ) -> Self {
        Self {
            queues: HashMap::new(),
            closed,
            handler,
            capacity: capacity.max(1),
            max_queues: max_queues.max(1),
        }
    }

    /// The queue a command belongs to: the resource the action mutates, which
    /// is not always the `device` string the client used. `connect_neon_rest`
    /// arrives as `neon_lsl` but replaces the `pupil` device, so it must be
    /// ordered with pupil markers; phone discovery touches no device at all and
    /// must not hold pupil markers for its browse window.
    pub fn key_for(device: &str, action: &CommandAction) -> String {
        match action {
            CommandAction::ConnectNeonRest => "pupil".to_string(),
            CommandAction::DiscoverNeonPhones => "neon_discovery".to_string(),
            _ => device.to_lowercase(),
        }
    }

    pub fn dispatch(&mut self, cmd: QueuedCommand) -> Dispatch {
        let key = Self::key_for(&cmd.device, &cmd.action);
        if !self.queues.contains_key(&key) && self.queues.len() >= self.max_queues {
            return Dispatch::TooManyQueues(cmd);
        }
        let capacity = self.capacity;
        let closed = self.closed.clone();
        let handler = self.handler.clone();
        let sender = self.queues.entry(key.clone()).or_insert_with(|| {
            let (qtx, mut qrx) = mpsc::channel::<QueuedCommand>(capacity);
            tokio::spawn(async move {
                while let Some(cmd) = qrx.recv().await {
                    if closed.load(Ordering::Relaxed) {
                        debug!(
                            "Connection closed; dropping queued {:?} for {}",
                            cmd.action, cmd.device
                        );
                        continue;
                    }
                    (handler)(cmd).await;
                }
                debug!("Device queue worker for {} finished", key);
            });
            qtx
        });
        match sender.try_send(cmd) {
            Ok(()) => Dispatch::Queued,
            Err(mpsc::error::TrySendError::Full(cmd)) => {
                warn!(
                    "Command queue for {} is full ({} pending); rejecting {:?}",
                    cmd.device, capacity, cmd.action
                );
                Dispatch::Full(cmd)
            }
            Err(mpsc::error::TrySendError::Closed(cmd)) => Dispatch::WorkerGone(cmd),
        }
    }

    /// Number of distinct device queues created so far.
    pub fn len(&self) -> usize {
        self.queues.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queues.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::time::Duration;
    use tokio::sync::Notify;

    fn cmd(device: &str, action: CommandAction, id: &str) -> QueuedCommand {
        QueuedCommand {
            device: device.to_string(),
            action,
            payload: None,
            id: Some(id.to_string()),
        }
    }

    /// Handler that records ids in completion order and optionally blocks on a
    /// per-device gate so tests can control interleaving.
    fn recording_handler(
        log: Arc<Mutex<Vec<String>>>,
        gate: Option<(String, Arc<Notify>)>,
    ) -> CommandHandler {
        Arc::new(move |c: QueuedCommand| {
            let log = log.clone();
            let gate = gate.clone();
            Box::pin(async move {
                if let Some((device, notify)) = gate {
                    if c.device == device {
                        notify.notified().await;
                    }
                }
                log.lock().unwrap().push(c.id.unwrap_or_default());
            })
        })
    }

    async fn settle() {
        for _ in 0..20 {
            tokio::task::yield_now().await;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    #[test]
    fn key_follows_the_mutated_resource_not_the_client_string() {
        assert_eq!(
            DeviceQueues::key_for("Kernel", &CommandAction::SendEvent),
            "kernel"
        );
        assert_eq!(
            DeviceQueues::key_for("neon_lsl", &CommandAction::ConnectNeonRest),
            "pupil"
        );
        assert_eq!(
            DeviceQueues::key_for("pupil", &CommandAction::DiscoverNeonPhones),
            "neon_discovery"
        );
        assert_eq!(
            DeviceQueues::key_for("neon_lsl", &CommandAction::ConnectNeonGaze),
            "neon_lsl"
        );
    }

    #[tokio::test]
    async fn commands_for_one_device_run_in_arrival_order() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut q = DeviceQueues::new(
            Arc::new(AtomicBool::new(false)),
            recording_handler(log.clone(), None),
        );
        for i in 0..5 {
            assert!(matches!(
                q.dispatch(cmd("kernel", CommandAction::SendEvent, &format!("k{i}"))),
                Dispatch::Queued
            ));
        }
        // A disconnect queued after the markers must run after them.
        assert!(matches!(
            q.dispatch(cmd("kernel", CommandAction::Disconnect, "k-disc")),
            Dispatch::Queued
        ));
        settle().await;
        assert_eq!(
            *log.lock().unwrap(),
            vec!["k0", "k1", "k2", "k3", "k4", "k-disc"]
        );
    }

    #[tokio::test]
    async fn a_stalled_device_does_not_delay_another() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let gate = Arc::new(Notify::new());
        let mut q = DeviceQueues::new(
            Arc::new(AtomicBool::new(false)),
            recording_handler(log.clone(), Some(("kernel".to_string(), gate.clone()))),
        );
        q.dispatch(cmd("kernel", CommandAction::SendEvent, "k0"));
        q.dispatch(cmd("pupil", CommandAction::SendEvent, "p0"));
        q.dispatch(cmd("pupil", CommandAction::SendEvent, "p1"));
        settle().await;
        // Kernel is blocked on its gate; pupil markers must have gone through.
        assert_eq!(*log.lock().unwrap(), vec!["p0", "p1"]);
        gate.notify_one();
        settle().await;
        assert_eq!(*log.lock().unwrap(), vec!["p0", "p1", "k0"]);
    }

    #[tokio::test]
    async fn queued_commands_are_dropped_once_the_connection_closes() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let gate = Arc::new(Notify::new());
        let closed = Arc::new(AtomicBool::new(false));
        let mut q = DeviceQueues::new(
            closed.clone(),
            recording_handler(log.clone(), Some(("pupil".to_string(), gate.clone()))),
        );
        q.dispatch(cmd("pupil", CommandAction::SendEvent, "in-flight"));
        q.dispatch(cmd("pupil", CommandAction::SendEvent, "queued-1"));
        q.dispatch(cmd("pupil", CommandAction::SendEvent, "queued-2"));
        settle().await;
        // The first command is in flight (blocked on the gate); the socket closes.
        closed.store(true, Ordering::Relaxed);
        gate.notify_one();
        settle().await;
        // The in-flight command finished; the ones still queued were dropped, so
        // a reconnecting client that retries them cannot produce duplicates.
        assert_eq!(*log.lock().unwrap(), vec!["in-flight"]);
    }

    #[tokio::test]
    async fn a_full_queue_rejects_the_new_command_and_keeps_order_of_accepted_ones() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let gate = Arc::new(Notify::new());
        let mut q = DeviceQueues::with_limits(
            Arc::new(AtomicBool::new(false)),
            recording_handler(log.clone(), Some(("kernel".to_string(), gate.clone()))),
            2,
            MAX_DEVICE_QUEUES,
        );
        q.dispatch(cmd("kernel", CommandAction::SendEvent, "k0")); // taken by the worker, blocked
        settle().await;
        assert!(matches!(
            q.dispatch(cmd("kernel", CommandAction::SendEvent, "k1")),
            Dispatch::Queued
        ));
        assert!(matches!(
            q.dispatch(cmd("kernel", CommandAction::SendEvent, "k2")),
            Dispatch::Queued
        ));
        match q.dispatch(cmd("kernel", CommandAction::SendEvent, "k3")) {
            Dispatch::Full(rejected) => assert_eq!(rejected.id.as_deref(), Some("k3")),
            other => panic!("expected Full, got {other:?}"),
        }
        // Release the worker: everything accepted still runs in order.
        gate.notify_one();
        settle().await;
        gate.notify_one();
        settle().await;
        gate.notify_one();
        settle().await;
        assert_eq!(*log.lock().unwrap(), vec!["k0", "k1", "k2"]);
    }

    #[tokio::test]
    async fn the_number_of_queues_is_bounded() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut q = DeviceQueues::with_limits(
            Arc::new(AtomicBool::new(false)),
            recording_handler(log.clone(), None),
            DEVICE_QUEUE_CAPACITY,
            2,
        );
        assert!(matches!(
            q.dispatch(cmd("a", CommandAction::Send, "a0")),
            Dispatch::Queued
        ));
        assert!(matches!(
            q.dispatch(cmd("b", CommandAction::Send, "b0")),
            Dispatch::Queued
        ));
        assert!(matches!(
            q.dispatch(cmd("c", CommandAction::Send, "c0")),
            Dispatch::TooManyQueues(_)
        ));
        // Existing keys still accept.
        assert!(matches!(
            q.dispatch(cmd("a", CommandAction::Send, "a1")),
            Dispatch::Queued
        ));
        assert_eq!(q.len(), 2);
    }
}
