#[test]
fn background_lifecycle() {
    use settings::ProfileSyncState::{Conflict, Synced};
    use tool_adapter::{RemoteToolId::Codex, RemoteToolInstallState};

    let (generation, original_file) = install_post_connect("fixture-a", 27891);
    start_post_connect(generation);
    wait_for_post_connect(1);
    assert_eq!(fs::read_to_string(&original_file).unwrap(), "27891");
    {
        let mut state = lock().unwrap();
        assert_eq!(state.summary.session_environment_state, RuntimeState::Ready);
        assert_eq!(state.summary.post_connect_status, PostConnectStatus::Ready);
        assert_eq!(
            state.summary.tool_installations[0].state,
            RemoteToolInstallState::Missing
        );
        let profile = ProfileSyncTask::claim(&mut state, Codex).unwrap();
        drop(state);
        finish_profile_sync(profile, Conflict);
        finish_profile_sync(profile, Synced);
        let state = lock().unwrap();
        assert!(state.profile_syncing.is_none());
        assert_eq!(state.summary.codex_profile_state, Conflict);
        assert!(state.environment_write.is_none());
    }
    disconnect(true).unwrap();
    disconnect(true).unwrap();
    assert!(!original_file.exists());
    let count = OPERATIONS
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, op)| op == "session-env-remove")
        .count();
    assert_eq!(count, 1);

    for (finished, old_port) in [(3, 27890), (5, 27892)] {
        let (env_arrived, env_release) = pause_once("session-env-apply:fixture-a");
        let (tools_arrived, tools_release) = pause_once("tool-discover:fixture-a");
        let (generation, old_file) = install_post_connect("fixture-a", old_port);
        start_post_connect(generation);
        env_arrived.recv_timeout(LIMIT).unwrap();
        tools_arrived.recv_timeout(LIMIT).unwrap();
        let old_profile = ProfileSyncTask::claim(&mut lock().unwrap(), Codex).unwrap();
        let offset = OPERATIONS.lock().unwrap().len();
        disconnect(true).unwrap();
        disconnect(true).unwrap();
        assert_eq!(
            OPERATIONS.lock().unwrap().len(),
            offset,
            "cleanup must wait for the old write"
        );
        assert!(!old_file.exists());

        let (generation, new_file) = install_post_connect("fixture-b", 27893);
        start_post_connect(generation);
        wait_for_post_connect(finished - 1);
        let new_profile = ProfileSyncTask::claim(&mut lock().unwrap(), Codex).unwrap();
        finish_profile_sync(old_profile, Conflict);
        tools_release.send(()).unwrap();
        env_release.send(()).unwrap();
        wait_for_post_connect(finished);

        let state = lock().unwrap();
        assert_eq!(state.summary.target.as_ref().unwrap().id, "fixture-b");
        assert_eq!(state.summary.session_environment_state, RuntimeState::Ready);
        assert_eq!(
            state.summary.tool_installations[0].state,
            RemoteToolInstallState::Missing
        );
        assert_eq!(state.profile_syncing, Some(new_profile));
        assert!(state
            .summary
            .timings
            .iter()
            .all(|phase| phase.phase != "remoteTools.discovery" || phase.outcome == "success"));
        assert!(state.environment_write.is_none());
        drop(state);
        assert_eq!(fs::read_to_string(&new_file).unwrap(), "27893");
        assert!(
            !old_file.exists(),
            "deferred cleanup left the old session file"
        );
        let operations = OPERATIONS.lock().unwrap();
        let old: Vec<_> = operations[offset..]
            .iter()
            .filter(|(id, op)| id == "fixture-a" && op.starts_with("session-env"))
            .map(|(_, op)| op.as_str())
            .collect();
        assert_eq!(old, ["session-env-apply", "session-env-remove"]);
        drop(operations);
        finish_profile_sync(new_profile, Synced);
        disconnect(true).unwrap();
        assert!(!new_file.exists());
    }
    println!("PASS normal/duplicate cleanup, stale successful/failed writes, tool discovery and Profile isolation");
}
