// Append to remote_bridge/tests.rs in a disposable candidate checkout.
// Each case runs in its own test process because shutdown is permanent.
mod r46_lifecycle_regressions {
    use super::super::*;
    use std::sync::mpsc::{self, Sender};

    const DEADLINE: Duration = Duration::from_secs(3);

    struct Child {
        name: &'static str,
        released: Sender<&'static str>,
        resume_drop: Option<mpsc::Receiver<()>>,
    }

    impl ssh::ManagedSsh for Child {
        fn is_running(&mut self) -> BridgeResult<bool> {
            Ok(true)
        }
    }

    impl Drop for Child {
        fn drop(&mut self) {
            self.released.send(self.name).unwrap();
            if let Some(resume) = &self.resume_drop {
                resume.recv_timeout(DEADLINE).unwrap();
            }
        }
    }

    fn install_transport(
        state: &mut Store,
        released: &Sender<&'static str>,
        resume_drop: Option<mpsc::Receiver<()>>,
    ) {
        state.child = Some(Box::new(Child {
            name: "primary",
            released: released.clone(),
            resume_drop,
        }));
        state.additional_tunnels.push(Box::new(Child {
            name: "additional",
            released: released.clone(),
            resume_drop: None,
        }));
        state.summary.status = Status::Connected;
    }

    fn assert_new_process() {
        assert!(!STOPPING.load(std::sync::atomic::Ordering::SeqCst));
        assert_terminal();
    }

    fn assert_terminal() {
        assert_eq!(store().report().status, Status::Disconnected);
        let tray = tray_state::snapshot();
        assert_eq!(tray.connection, tray_state::Connection::Disconnected);
        assert!(!tray.can_change);
    }

    fn assert_released_once(released: &mpsc::Receiver<&'static str>) {
        assert_eq!(released.recv_timeout(DEADLINE).unwrap(), "primary");
        assert_eq!(released.recv_timeout(DEADLINE).unwrap(), "additional");
        assert_eq!(
            released.recv_timeout(DEADLINE),
            Err(mpsc::RecvTimeoutError::Disconnected)
        );
    }

    #[test]
    fn shutdown_retires_transport_once_and_publishes_terminal_state() {
        assert_new_process();
        let (released, receipts) = mpsc::channel();
        install_transport(&mut lock().unwrap(), &released, None);
        assert_eq!(store().report().status, Status::Connected);
        assert_eq!(
            tray_state::snapshot().connection,
            tray_state::Connection::Connected
        );
        drop(released);
        shutdown();
        shutdown();
        assert_released_once(&receipts);
        assert_terminal();
        let state = lock().unwrap();
        assert_eq!(state.connection_generation, 1);
        assert!(state.child.is_none());
        assert!(state.additional_tunnels.is_empty());
    }

    #[test]
    fn shutdown_returns_while_store_is_held_and_reaps_after_release() {
        assert_new_process();
        let (released, receipts) = mpsc::channel();
        let mut state = lock().unwrap();
        install_transport(&mut state, &released, None);
        drop(released);
        let (done, completion) = mpsc::channel();
        let caller = std::thread::spawn(move || {
            shutdown();
            shutdown();
            done.send(()).unwrap();
        });
        let result = completion.recv_timeout(DEADLINE);
        assert!(receipts.try_recv().is_err());
        drop(state);
        result.unwrap();
        caller.join().unwrap();
        assert_released_once(&receipts);
        assert_terminal();
    }

    #[test]
    fn shutdown_drops_resources_outside_store_after_publishing() {
        assert_new_process();
        let (released, receipts) = mpsc::channel();
        let (resume, paused_drop) = mpsc::channel();
        install_transport(&mut lock().unwrap(), &released, Some(paused_drop));
        drop(released);
        shutdown();
        assert_eq!(receipts.recv_timeout(DEADLINE).unwrap(), "primary");
        assert!(store().try_lock().unwrap().child.is_none());
        assert_terminal();
        assert!(receipts.try_recv().is_err());
        resume.send(()).unwrap();
        assert_eq!(receipts.recv_timeout(DEADLINE).unwrap(), "additional");
        assert_eq!(
            receipts.recv_timeout(DEADLINE),
            Err(mpsc::RecvTimeoutError::Disconnected)
        );
    }
}
