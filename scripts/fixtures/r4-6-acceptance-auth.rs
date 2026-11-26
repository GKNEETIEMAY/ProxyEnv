// Included only in the disposable archive for final lifecycle acceptance.
#[cfg(test)]
pub(super) mod acceptance_auth {
    use super::*;
    use std::io;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Clone, Debug)]
    struct SyntheticChild(Arc<AtomicUsize>);

    impl portable_pty::ChildKiller for SyntheticChild {
        fn kill(&mut self) -> io::Result<()> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn clone_killer(&self) -> Box<dyn portable_pty::ChildKiller + Send + Sync> {
            Box::new(self.clone())
        }
    }

    impl Child for SyntheticChild {
        fn try_wait(&mut self) -> io::Result<Option<portable_pty::ExitStatus>> {
            Ok(None)
        }

        fn wait(&mut self) -> io::Result<portable_pty::ExitStatus> {
            Ok(portable_pty::ExitStatus::with_exit_code(0))
        }

        fn process_id(&self) -> Option<u32> {
            None
        }

        #[cfg(windows)]
        fn as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle> {
            None
        }
    }

    pub(in crate::features::remote_bridge) fn stage(
        id: &str,
        target: super::super::RemoteTarget,
        fingerprint: String,
    ) -> Arc<AtomicUsize> {
        let drops = Arc::new(AtomicUsize::new(0));
        // The master is an anonymous local PTY. No shell or SSH process is spawned.
        let pair = native_pty_system().openpty(PtySize::default()).unwrap();
        let writer: SharedWriter = Arc::new(Mutex::new(Box::new(io::sink())));
        let bytes = b"fixture-marker\r\n{\"verified\":true}\r\n".to_vec();
        let byte_count = u64::try_from(bytes.len()).unwrap();
        let now = Instant::now();
        let session = Session {
            id: id.into(),
            target_id: target.id.clone(),
            operation: Operation::Connect,
            auth: SshAuthState {
                mode: SshAuthMode::Interactive,
                method: SshAuthMethod::Agent,
                authenticated: true,
                password_stored: false,
            },
            status: SessionStatus::Succeeded,
            error: None,
            output: Arc::new(Mutex::new(TerminalOutput {
                bytes_received: byte_count,
                printable_bytes: byte_count,
                bytes,
                authenticated: true,
                marker: b"fixture-marker".to_vec(),
                pending_echo: None,
                echo_match_len: 0,
                cpr_requests: 0,
                remote_result_detected: true,
                closed: false,
            })),
            writer: writer.clone(),
            process: Some(PtyProcess {
                child: Box::new(SyntheticChild(drops.clone())),
                _master: pair.master,
                _writer: writer,
            }),
            completion: Completion::Connect {
                request: Request {
                    target_id: target.id.clone(),
                    proxy_port: None,
                    cc_port: Some(25721),
                    cc_local_port: 15721,
                    expected_revision: 0,
                },
                summary: Box::new(Summary {
                    target: Some(target),
                    ..Summary::default()
                }),
                fingerprint: fingerprint.clone(),
                proxy_relay: Box::new(None),
                ai_relay: Box::new(None),
            },
            created_at: now,
            attempt: 1,
            current_prompt: None,
            last_prompt: None,
            prompt_generation: 0,
            prompt_consumed_at: 0,
            observed_output_len: 0,
            waiting_since: now,
            authenticated_at: Some(now),
            target_fingerprint: fingerprint,
            password_candidate: None,
            password_cache_eligible: false,
            cached_password_attempted: false,
        };
        super::super::mark_interactive_connecting().unwrap();
        assert!(lock_sessions()
            .unwrap()
            .insert(id.into(), session)
            .is_none());
        drops
    }
}
