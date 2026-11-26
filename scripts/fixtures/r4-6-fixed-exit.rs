    fn save_exit_snapshot(dir: &Path) {
        let status = format!(
            "{:?} {:?}",
            report().status,
            tray_state::snapshot().connection
        );
        fs::write(dir.join("callback.result"), status).unwrap();
    }

    fn run_exit_callback(dir: &Path) {
        let (finished, completion) = mpsc::channel();
        assert!(coordinate_shutdown(move |outcome| finished.send(outcome).unwrap()));
        assert!(!coordinate_shutdown(|_| panic!("duplicate cleanup callback")));
        mark(dir, "shutdown.requested");
        let outcome = completion.recv_timeout(LIMIT).unwrap();
        let serialized = serde_json::to_vec(&outcome).unwrap();
        fs::write(dir.join("native-outcome.json"), serialized).unwrap();
        shutdown();
        save_exit_snapshot(dir);
    }

    fn assert_shutdown_result(
        dir: &Path,
        mode: &str,
        exit_state: &str,
        worker_finished: bool,
        drops: [bool; 4],
    ) {
        let log = fs::read_to_string(dir.join("logs/bridge-0.jsonl")).unwrap();
        let outcomes: Vec<_> = log
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .filter(|event| event["phase"] == "bridge.shutdown")
            .map(|event| event["outcome"].clone())
            .collect();
        assert_eq!(outcomes.len(), 1, "one application exit outcome");
        if let Ok(native) = fs::read_to_string(dir.join("native-outcome.json")) {
            assert_eq!(serde_json::from_str::<serde_json::Value>(&native).unwrap(), outcomes[0]);
        }
        match mode {
            "normal" | "busy" | "contended" => {
                assert_eq!(outcomes[0], "completed");
                assert_eq!(exit_state, "Disconnected Disconnected");
                assert!(worker_finished);
                assert_eq!(drops, [true; 4]);
                if mode == "busy" {
                    assert!(dir.join("work.done").exists());
                }
            }
            "timeout" | "lock-timeout" => {
                assert_eq!(outcomes[0], "timedOut");
                assert!(!worker_finished);
                if mode == "timeout" {
                    assert_eq!(exit_state, "Disconnected Disconnected");
                    assert_eq!(drops, [true, false, false, false]);
                } else {
                    assert_eq!(exit_state, "Connected Connected");
                    assert_eq!(drops, [false; 4]);
                }
            }
            _ => panic!("unknown exit test mode"),
        }
        println!("PASS {mode} persisted outcome={} and observed resource state", outcomes[0]);
    }
