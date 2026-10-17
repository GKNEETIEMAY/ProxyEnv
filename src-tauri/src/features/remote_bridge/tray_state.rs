//! Cached read model of the existing bridge store, published on state changes.
//! Never probe SSH, discover endpoints, or expose target/session information here.
use super::{reconnect, ssh_auth, store, PostConnectStatus, Status, Store, Summary};
use std::sync::{mpsc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Connection {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Attention,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Snapshot {
    pub connection: Connection,
    pub proxy: bool,
    pub cc: bool,
    pub can_change: bool,
    pub attention_generation: Option<u64>,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            connection: Connection::Disconnected,
            proxy: false,
            cc: false,
            can_change: false,
            attention_generation: None,
        }
    }
}

impl Snapshot {
    fn project(summary: &Summary, has_transport: bool, busy: bool) -> Self {
        let connection = match summary.reconnect_state {
            reconnect::State::AttentionRequired => Connection::Attention,
            reconnect::State::Waiting | reconnect::State::Retrying => Connection::Reconnecting,
            reconnect::State::Idle => match summary.status {
                Status::Disconnected => Connection::Disconnected,
                Status::Connecting => Connection::Connecting,
                Status::Connected if has_transport => Connection::Connected,
                _ => Connection::Attention,
            },
        };
        Self {
            connection,
            proxy: connection != Connection::Disconnected && summary.proxy.is_some(),
            cc: connection != Connection::Disconnected && summary.cc.is_some(),
            can_change: connection == Connection::Connected
                && summary.post_connect_status != PostConnectStatus::Preparing
                && !busy,
            attention_generation: None,
        }
    }
}

#[derive(Default)]
struct Cached {
    snapshot: Snapshot,
    listeners: Vec<mpsc::SyncSender<()>>,
}

/// Session-only, sanitized state. Slow consumers coalesce notifications and read
/// the newest snapshot; no event backlog, timers, or network work is needed.
#[derive(Default)]
pub(super) struct Cache(Mutex<Cached>);

impl Cache {
    fn snapshot(&self) -> Snapshot {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).snapshot
    }

    fn subscribe(&self) -> mpsc::Receiver<()> {
        let (sender, receiver) = mpsc::sync_channel(1);
        self.listen(sender);
        receiver
    }

    fn listen(&self, sender: mpsc::SyncSender<()>) {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .listeners
            .push(sender);
    }

    pub(super) fn publish(&self, state: &Store) {
        let mut snapshot = Snapshot::project(
            &state.summary,
            state.child.is_some(),
            state.profile_syncing
                || state.pending.is_some()
                || state.extension_pending.is_some()
                || state.reconnect.is_some(),
        );
        snapshot.attention_generation = (state.summary.reconnect_state
            == reconnect::State::AttentionRequired)
            .then_some(state.connection_generation);
        let mut cache = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if cache.snapshot == snapshot {
            return;
        }
        cache.snapshot = snapshot;
        cache.listeners.retain(|listener| {
            !matches!(
                listener.try_send(()),
                Err(mpsc::TrySendError::Disconnected(()))
            )
        });
    }
}

pub(crate) fn snapshot() -> Snapshot {
    store().tray.snapshot()
}

pub(crate) fn subscribe() -> mpsc::Receiver<()> {
    store().tray.subscribe()
}

pub(crate) fn listen(sender: mpsc::SyncSender<()>) {
    store().tray.listen(sender);
}

/// Only actions revalidate the live safety gates. Display reads never compete
/// with Store or turn brief lock contention into a fake connection transition.
pub(crate) fn can_change() -> bool {
    // Release the update gate before reading Store; no Store -> gate lock inversion.
    let busy =
        crate::desktop::updates::bridge_changes_busy() || ssh_auth::update_busy().unwrap_or(true);
    live_can_change(store(), busy)
}

fn live_can_change(source: &super::StateStore, busy: bool) -> bool {
    let Ok(state) = source.try_lock() else {
        return false;
    };
    Snapshot::project(
        &state.summary,
        state.child.is_some(),
        busy || state.profile_syncing
            || state.pending.is_some()
            || state.extension_pending.is_some()
            || state.reconnect.is_some(),
    )
    .can_change
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connected(proxy: bool, cc: bool) -> Summary {
        let endpoint = super::super::Endpoint {
            local: super::super::ProxyEndpoint {
                host: "127.0.0.1".into(),
                port: 7897,
                protocol: super::super::ProxyProtocol::Mixed,
            },
            remote_port: 7897,
        };
        Summary {
            status: Status::Connected,
            proxy: proxy.then(|| endpoint.clone()),
            cc: cc.then_some(endpoint),
            ..Summary::default()
        }
    }

    #[test]
    fn transport_and_capabilities_are_independent() {
        for proxy in [false, true] {
            for cc in [false, true] {
                let state = Snapshot::project(&connected(proxy, cc), true, false);
                assert_eq!(state.connection, Connection::Connected);
                assert_eq!((state.proxy, state.cc), (proxy, cc));
                assert!(state.can_change);
            }
        }
    }

    #[test]
    fn unsafe_and_incomplete_states_cannot_change_capabilities() {
        let mut summary = connected(true, false);
        assert!(!Snapshot::project(&summary, false, false).can_change);
        assert!(!Snapshot::project(&summary, true, true).can_change);
        summary.post_connect_status = PostConnectStatus::Preparing;
        assert!(!Snapshot::project(&summary, true, false).can_change);
        for status in [
            Status::Disconnected,
            Status::Connecting,
            Status::Stale,
            Status::Unavailable,
            Status::Error,
        ] {
            summary.status = status;
            assert!(!Snapshot::project(&summary, true, false).can_change);
        }
        let disconnected = Snapshot::project(&Summary::default(), false, false);
        assert_eq!(disconnected.connection, Connection::Disconnected);
        assert_eq!((disconnected.proxy, disconnected.cc), (false, false));
        assert_eq!(disconnected, Snapshot::default());
    }

    #[test]
    fn reconnect_and_attention_take_priority_over_last_connected_status() {
        let mut summary = connected(true, true);
        for reconnect in [
            reconnect::State::Waiting,
            reconnect::State::Retrying,
            reconnect::State::AttentionRequired,
        ] {
            summary.reconnect_state = reconnect;
            let state = Snapshot::project(&summary, true, false);
            assert!(!state.can_change);
            assert_eq!(
                state.connection,
                if reconnect == reconnect::State::AttentionRequired {
                    Connection::Attention
                } else {
                    Connection::Reconnecting
                }
            );
        }
    }

    struct UnprobedTransport;
    impl super::super::ssh::ManagedSsh for UnprobedTransport {
        fn is_running(&mut self) -> super::super::BridgeResult<bool> {
            panic!("Reading or publishing tray state must not probe SSH");
        }
    }

    fn connect(source: &super::super::StateStore) {
        let mut state = source.lock().unwrap();
        state.summary = connected(true, false);
        state.child = Some(Box::new(UnprobedTransport));
    }

    #[test]
    fn cache_reads_survive_store_contention_but_actions_fail_closed() {
        let source = super::super::StateStore::default();
        connect(&source);
        let expected = source.tray.snapshot();
        assert!(expected.can_change);
        assert!(live_can_change(&source, false));
        assert!(!live_can_change(&source, true));
        let _operation = source.lock().unwrap();
        assert!(source.try_lock().is_err());
        for _ in 0..5 {
            assert_eq!(source.tray.snapshot(), expected);
            assert!(!live_can_change(&source, false));
        }
    }

    #[test]
    fn state_release_publishes_real_transitions_and_independent_capabilities() {
        let source = super::super::StateStore::default();
        let changes = source.tray.subscribe();
        assert_eq!(source.tray.snapshot(), Snapshot::default());
        {
            let mut state = source.lock().unwrap();
            state.summary.status = Status::Connecting;
            assert!(changes.try_recv().is_err());
        }
        changes.try_recv().unwrap();
        assert_eq!(source.tray.snapshot().connection, Connection::Connecting);
        connect(&source);
        changes.try_recv().unwrap();
        assert_eq!(
            (source.tray.snapshot().proxy, source.tray.snapshot().cc),
            (true, false)
        );
        {
            let mut state = source.lock().unwrap();
            state.summary = connected(false, true);
        }
        changes.try_recv().unwrap();
        assert_eq!(
            (source.tray.snapshot().proxy, source.tray.snapshot().cc),
            (false, true)
        );
        {
            let mut state = source.try_lock().unwrap();
            state.summary.reconnect_state = reconnect::State::Retrying;
        }
        changes.try_recv().unwrap();
        assert_eq!(source.tray.snapshot().connection, Connection::Reconnecting);
        {
            let mut state = source.lock().unwrap();
            state.summary = Summary::default();
            state.child = None;
        }
        changes.try_recv().unwrap();
        assert_eq!(source.tray.snapshot(), Snapshot::default());
    }

    #[test]
    fn notifications_deduplicate_and_slow_consumers_read_latest_not_an_event_backlog() {
        let source = super::super::StateStore::default();
        let first = source.tray.subscribe();
        let second = source.tray.subscribe();
        let closed = source.tray.subscribe();
        drop(closed);
        connect(&source);
        assert_eq!(source.tray.0.lock().unwrap().listeners.len(), 2);
        first.try_recv().unwrap();
        drop(source.lock().unwrap());
        assert!(first.try_recv().is_err(), "Unchanged reads must not notify");
        {
            let mut state = source.try_lock().unwrap();
            state.summary.status = Status::Disconnected;
        }
        // second already has a queued wakeup, but always reads the newest state.
        second.try_recv().unwrap();
        assert!(second.try_recv().is_err());
        first.try_recv().unwrap();
        assert_eq!(source.tray.snapshot(), Snapshot::default());
    }

    #[test]
    fn only_manual_reconnect_attention_publishes_an_episode_and_resolution_clears_it() {
        let source = super::super::StateStore::default();
        let changes = source.tray.subscribe();
        {
            let mut state = source.lock().unwrap();
            state.connection_generation = 9;
            state.summary.status = Status::Stale;
        }
        changes.try_recv().unwrap();
        assert_eq!(source.tray.snapshot().attention_generation, None);
        {
            let mut state = source.lock().unwrap();
            state.summary.reconnect_state = reconnect::State::AttentionRequired;
        }
        changes.try_recv().unwrap();
        assert_eq!(source.tray.snapshot().attention_generation, Some(9));
        {
            let mut state = source.lock().unwrap();
            state.connection_generation = 10;
        }
        changes.try_recv().unwrap();
        assert_eq!(source.tray.snapshot().attention_generation, Some(10));
        {
            let mut state = source.lock().unwrap();
            state.summary = Summary::default();
        }
        changes.try_recv().unwrap();
        assert_eq!(source.tray.snapshot().attention_generation, None);
    }
}
