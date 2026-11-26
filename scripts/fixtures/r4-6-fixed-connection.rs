#[test]
fn connection_lifecycle() {
    use ssh_auth::{acceptance_auth::stage, cancel, finish};

    let normal = stage("normal", target("fixture-a"), fingerprint("fixture-a"));
    assert!(finish("normal").is_ok());
    wait_for_post_connect(1);
    assert_eq!(connected_target().as_deref(), Some("fixture-a"));
    assert_eq!(
        finish("normal").err().as_deref(),
        Some("sshAuthSessionMissing")
    );
    assert_eq!(normal.load(SeqCst), 0);
    disconnect(true).unwrap();
    assert_eq!(normal.load(SeqCst), 1);

    let cancelled = stage("cancelled", target("fixture-a"), fingerprint("fixture-a"));
    cancel("cancelled").unwrap();
    assert_eq!(
        finish("cancelled").err().as_deref(),
        Some("sshAuthSessionMissing")
    );
    assert_eq!(cancelled.load(SeqCst), 1);

    for (id, cancel_after_take) in [("cancel-taken", true), ("disconnect-taken", false)] {
        let old = stage(id, target("fixture-a"), fingerprint("fixture-a"));
        let (arrived, release) = pause_once(id);
        let task = thread::spawn(move || finish(id));
        arrived.recv_timeout(LIMIT).unwrap();
        if cancel_after_take {
            cancel(id).unwrap();
        } else {
            disconnect(true).unwrap();
        }
        release.send(()).unwrap();
        assert_eq!(
            task.join().unwrap().err().as_deref(),
            Some("sshAuthSessionMissing")
        );
        assert_eq!(old.load(SeqCst), 1);
        assert_eq!(report().status, Status::Disconnected);
        assert!(connected_target().is_none());
    }

    for (expected, newest_target) in [(2, "fixture-b"), (3, "fixture-a")] {
        let (arrived, release) = pause_once("old-a");
        let old = stage("old-a", target("fixture-a"), fingerprint("fixture-a"));
        let task = thread::spawn(|| finish("old-a"));
        arrived.recv_timeout(LIMIT).unwrap();
        let next = stage("new-b", target("fixture-b"), fingerprint("fixture-b"));
        let newest = if newest_target == "fixture-a" {
            stage("new-a", target("fixture-a"), fingerprint("fixture-a"))
        } else {
            next.clone()
        };
        if newest_target == "fixture-a" {
            // In A/B/A, cancelling B also must leave the newest A intact.
            cancel("new-b").unwrap();
            assert_eq!(next.load(SeqCst), 1);
        }
        release.send(()).unwrap();
        assert_eq!(
            task.join().unwrap().err().as_deref(),
            Some("sshAuthSessionMissing")
        );
        assert_eq!(old.load(SeqCst), 1);
        assert_eq!(report().status, Status::Connecting);
        assert!(finish(if newest_target == "fixture-a" {
            "new-a"
        } else {
            "new-b"
        })
        .is_ok());
        wait_for_post_connect(expected);
        assert_eq!(connected_target().as_deref(), Some(newest_target));
        assert_eq!(newest.load(SeqCst), 0);
        disconnect(true).unwrap();
        assert_eq!(newest.load(SeqCst), 1);
    }

    let (arrived, release) = pause_once("during-stop");
    let old = stage("during-stop", target("fixture-a"), fingerprint("fixture-a"));
    let task = thread::spawn(|| finish("during-stop"));
    arrived.recv_timeout(LIMIT).unwrap();
    shutdown();
    release.send(()).unwrap();
    assert_eq!(
        task.join().unwrap().err().as_deref(),
        Some("sshAuthSessionMissing")
    );
    assert_eq!(old.load(SeqCst), 1);
    assert_eq!(report().status, Status::Disconnected);
    assert_eq!(
        tray_state::snapshot().connection,
        tray_state::Connection::Disconnected
    );
    assert_eq!(POST_CONNECT_DONE.load(SeqCst), 3);
    println!(
        "PASS connection, duplicate receipt, cancellation, disconnect, A/B, A/B/A and shutdown"
    );
}
