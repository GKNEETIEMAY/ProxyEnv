// Append to remote_bridge/tests.rs in a disposable baseline checkout only.
// Run each probe in a separate process with a watchdog. A passing assertion
// records the observed baseline behavior, including known defects.
mod r46b_lifecycle_probes {
    use super::super::*;
    use std::sync::atomic::Ordering;
    use std::sync::mpsc::{self, RecvTimeoutError, Sender};
    use std::thread;
    use std::time::{Duration, Instant};

    const DEADLINE: Duration = Duration::from_secs(3);
    const CONTENTION_WINDOW: Duration = Duration::from_millis(500);

    struct ObservedChild {
        name: &'static str,
        released: Sender<&'static str>,
    }

    impl ssh::ManagedSsh for ObservedChild {
        fn is_running(&mut self) -> BridgeResult<bool> {
            Ok(true)
        }
    }

    impl Drop for ObservedChild {
        fn drop(&mut self) {
            let _ = self.released.send(self.name);
        }
    }

    fn install_transport(state: &mut Store, released: &Sender<&'static str>) {
        state.child = Some(Box::new(ObservedChild {
            name: "primary",
            released: released.clone(),
        }));
        state.additional_tunnels.push(Box::new(ObservedChild {
            name: "additional",
            released: released.clone(),
        }));
        state.summary.status = Status::Connected;
        state.profile_syncing = true;
    }

    #[test]
    fn shutdown_under_store_contention() {
        let (released_tx, released_rx) = mpsc::channel();
        let mut held = lock().unwrap();
        install_transport(&mut held, &released_tx);
        let (start_tx, start_rx) = mpsc::sync_channel(0);
        let (done_tx, done_rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            start_rx.recv_timeout(DEADLINE).unwrap();
            shutdown();
            done_tx.send(()).unwrap();
        });
        start_tx.send(()).unwrap();
        let limit = Instant::now() + DEADLINE;
        while !STOPPING.load(Ordering::SeqCst) && Instant::now() < limit {
            thread::yield_now();
        }
        assert!(STOPPING.load(Ordering::SeqCst));
        let before_release = done_rx.recv_timeout(CONTENTION_WINDOW);
        drop(held);
        assert_eq!(before_release, Err(RecvTimeoutError::Timeout));
        done_rx.recv_timeout(DEADLINE).unwrap();
        worker.join().unwrap();

        let state = lock().unwrap();
        assert!(state.child.is_some());
        assert_eq!(state.additional_tunnels.len(), 1);
        assert!(state.profile_syncing);
        assert!(released_rx.try_recv().is_err());
        drop(state);
        eprintln!("observed: shutdown waited for held Store and returned without removing owned transports");
    }

    #[test]
    fn repeated_shutdown_with_uncontended_store() {
        let (released_tx, released_rx) = mpsc::channel();
        install_transport(&mut lock().unwrap(), &released_tx);
        assert!(store().report().status == Status::Connected);
        assert!(tray_state::snapshot().connection == tray_state::Connection::Connected);

        shutdown();
        assert_eq!(released_rx.try_recv().unwrap(), "primary");
        assert_eq!(released_rx.try_recv().unwrap(), "additional");
        {
            let state = lock().unwrap();
            assert!(state.child.is_none());
            assert!(state.additional_tunnels.is_empty());
            assert!(!state.profile_syncing);
        }
        shutdown();
        assert!(released_rx.try_recv().is_err());
        assert!(store().report().status == Status::Connected);
        assert!(tray_state::snapshot().connection == tray_state::Connection::Attention);
        assert!(!tray_state::snapshot().can_change);
        eprintln!("observed: ordered transport disposal once; Report retained Connected while Tray projected Attention");
    }

    #[test]
    fn stale_profile_completion() {
        let old_generation = lock().unwrap().connection_generation;
        let (captured_tx, captured_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::sync_channel(0);
        let (done_tx, done_rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            captured_tx.send(()).unwrap();
            resume_rx.recv_timeout(DEADLINE).unwrap();
            finish_profile_sync(
                tool_adapter::RemoteToolId::Codex,
                settings::ProfileSyncState::InvalidLocalProfile,
            );
            done_tx.send(()).unwrap();
        });
        captured_rx.recv_timeout(DEADLINE).unwrap();

        let (released_tx, _released_rx) = mpsc::channel();
        {
            let mut state = lock().unwrap();
            reconnect::cancel(&mut state);
            assert_ne!(state.connection_generation, old_generation);
            install_transport(&mut state, &released_tx);
            state.summary.codex_profile_state = settings::ProfileSyncState::Synced;
        }
        resume_tx.send(()).unwrap();
        done_rx.recv_timeout(DEADLINE).unwrap();
        worker.join().unwrap();

        let state = lock().unwrap();
        assert_ne!(state.connection_generation, old_generation);
        assert!(state.child.is_some());
        assert!(!state.profile_syncing);
        assert!(
            state.summary.codex_profile_state == settings::ProfileSyncState::InvalidLocalProfile
        );
        eprintln!("observed: old Profile completion cleared the current busy flag and overwrote the current Profile status");
    }

    #[test]
    fn bridge_operation_admission_after_shutdown() {
        shutdown();
        assert!(STOPPING.load(Ordering::SeqCst));
        assert!(crate::desktop::updates::bridge_operation().is_ok());
        eprintln!("observed: the general bridge-operation gate admitted new work after shutdown");
    }
}
