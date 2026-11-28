use super::{Phase, Runtime, Task};
mod ai;
pub(in super::super) mod managed;
use crate::features::remote_bridge::{
    target_os::windows_bridge::safety::{
        frozen::{FrozenProfile, HostTrustEvidence},
        frozen_resources::{FrozenResources, PendingCleanup},
        owned_process::{CloseReport, OwnedProcess, PROCESS_WAIT_MS},
        plan_lease_scope,
        policy::{
            lifecycle::{Enrollment, Observation, PolicyLifecycle, State, StatusSource},
            ClockSample, LeaseScope, SignedRecord, VerifiedPolicy,
        },
        policy_storage::PolicyStorage,
        PreparedForward, Verification, WindowsBridgeInput, WindowsBridgePlan,
    },
    target_os::WindowsAiState,
    BridgeResult,
};
use std::{
    io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use {ai::AiSession, managed::ManagedPhase};

const CLOSE_BUDGET: Duration = Duration::from_millis(PROCESS_WAIT_MS as u64);
pub(in super::super) const COORDINATED_EXIT_BUDGET: Duration = Duration::from_secs(3);
pub(in super::super) const EXIT_FALLBACK_BUDGET: Duration = Duration::from_millis(200);

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(in super::super) enum End {
    Cancelled,
    ApplicationExit,
    Expired,
    Revoked,
    AuthorizationUnavailable,
    HistoryUnavailable,
    SetupFailed,
    ProcessExited,
}

#[derive(Debug, PartialEq, Eq)]
pub(in super::super) enum Completion {
    Complete,
    Partial,
    TimedOut,
    Failed,
}

#[derive(Debug)]
pub(in super::super) struct Shutdown {
    pub end: End,
    pub completion: Completion,
    pub process: Option<CloseReport>,
    pub terminal: Option<CloseReport>,
    pub ai: WindowsAiState,
    pub pending: Vec<PendingCleanup>,
    pub setup_error: Option<io::Error>,
}

#[derive(Default)]
struct LocalResources {
    files: Option<FrozenResources>,
    process: Option<OwnedProcess>,
    terminal: Option<OwnedProcess>,
    terminal_close: Option<CloseReport>,
    released_any: bool,
}

impl LocalResources {
    fn close_terminal(&mut self, deadline: Instant) -> Option<CloseReport> {
        if let Some(mut terminal) = self.terminal.take() {
            self.terminal_close = Some(terminal.close_until(deadline));
        }
        self.terminal_close
    }

    fn close(
        mut self,
        end: End,
        deadline: Instant,
        pending: Vec<PendingCleanup>,
        setup_error: Option<io::Error>,
    ) -> Shutdown {
        let terminal = self.close_terminal(deadline);
        let process = self
            .process
            .take()
            .map(|mut process| process.close_until(deadline));
        Self::summarize(
            end,
            deadline,
            [process, terminal],
            self.files.take(),
            self.released_any,
            pending,
            setup_error,
        )
    }

    fn summarize(
        end: End,
        deadline: Instant,
        processes: [Option<CloseReport>; 2],
        files: Option<FrozenResources>,
        released_before_close: bool,
        mut pending: Vec<PendingCleanup>,
        setup_error: Option<io::Error>,
    ) -> Shutdown {
        let reports = || processes.into_iter().flatten();
        let mut timed_out = reports().any(|process| process.timed_out);
        let mut progress = released_before_close || reports().any(|process| process.process_exited);
        let drained = reports().all(|process| process.process_exited && process.job_empty);
        if let Some(files) = files {
            // The process must be confirmed stopped before its frozen inputs can be removed.
            let (missing, released_any) = if drained {
                files.retire_until(deadline)
            } else {
                let kind = if timed_out {
                    io::ErrorKind::TimedOut
                } else {
                    io::ErrorKind::Other
                };
                (files.retire_unclosed(kind), false)
            };
            timed_out |= drained
                && (Instant::now() >= deadline
                    || missing
                        .iter()
                        .any(|entry| entry.error.kind() == io::ErrorKind::TimedOut));
            progress |= released_any;
            pending.extend(missing);
        } else if released_before_close && Instant::now() >= deadline {
            timed_out = true;
        }
        timed_out |= drained
            && pending
                .iter()
                .any(|entry| entry.error.kind() == io::ErrorKind::TimedOut);
        let process_reclaimed = reports().all(|process| {
            process.process_exited && process.job_empty && process.termination_error.is_none()
        });
        let completion = if timed_out {
            Completion::TimedOut
        } else if process_reclaimed && pending.is_empty() {
            Completion::Complete
        } else if progress {
            Completion::Partial
        } else {
            Completion::Failed
        };
        Shutdown {
            end,
            completion,
            process: processes[0].or(processes[1]),
            terminal: processes[1],
            ai: WindowsAiState::NotEnabled,
            pending,
            setup_error,
        }
    }
}

impl Drop for LocalResources {
    fn drop(&mut self) {
        if self.process.is_none() && self.terminal.is_none() && self.files.is_none() {
            return;
        }
        let owned = Self {
            process: self.process.take(),
            terminal: self.terminal.take(),
            terminal_close: self.terminal_close.take(),
            files: self.files.take(),
            released_any: false,
        };
        let report = owned.close(End::Cancelled, cleanup_deadline(), Vec::new(), None);
        if report.completion != Completion::Complete {
            tracing::warn!(completion = ?report.completion, pending = report.pending.len(),
                "offline runtime cleanup incomplete");
        }
    }
}

pub(in super::super) struct LocalRuntime<'p, 'review> {
    // Revocation precedes transport teardown even when the owner is dropped.
    ai: AiSession,
    runtime: Runtime<LocalResources, Shutdown>,
    plan: &'p WindowsBridgePlan<'review>,
    history: PolicyStorage,
    scope: LeaseScope,
    policy: PolicyLifecycle<'review>,
    profile: Option<(PathBuf, FrozenProfile)>,
    managed: ManagedPhase,
    transport_verified: bool,
}

pub(in super::super) struct LocalVerification {
    task: Task,
    proof: Verification<()>,
}

impl LocalVerification {
    pub fn listener_report(&mut self, bytes: &[u8], now: ClockSample) -> BridgeResult<()> {
        self.proof.listener_report(bytes, now)
    }

    pub fn receipt(&mut self, bytes: &[u8], eof: bool, now: ClockSample) -> BridgeResult<()> {
        self.proof.receipt(bytes, eof, now)
    }

    #[cfg(test)]
    pub(in super::super) fn fixture_proof(&self) -> &Verification<()> {
        &self.proof
    }
}

impl<'p, 'review> LocalRuntime<'p, 'review> {
    pub fn begin(
        plan: &'p WindowsBridgePlan<'review>,
        history: PolicyStorage,
        enrollment: Enrollment,
    ) -> BridgeResult<(Self, Task)> {
        let issuer = plan
            .policy
            .issuer
            .as_ref()
            .ok_or("windowsForwardPolicyUnreviewed")?;
        let scope = plan_lease_scope(plan, issuer);
        let policy =
            PolicyLifecycle::new(issuer, scope.clone(), plan.policy.expires_at, enrollment);
        let mut runtime = Runtime::default();
        let task = runtime.begin().ok_or("windowsForwardPolicyDenied")?;
        Ok((
            Self {
                ai: AiSession::default(),
                runtime,
                plan,
                history,
                scope,
                policy,
                profile: None,
                managed: ManagedPhase::Unprepared,
                transport_verified: false,
            },
            task,
        ))
    }

    pub fn prepare(
        &mut self,
        task: &Task,
        current: WindowsBridgeInput<'_>,
        now: ClockSample,
        path: &Path,
        trust: HostTrustEvidence<'_>,
    ) -> BridgeResult<()> {
        if !self.runtime.owns(task) || !matches!(self.runtime.phase, Phase::Preparing) {
            return Err("windowsForwardPolicyDenied".into());
        }
        let profile = self
            .plan
            .forward_args(current, now.unix_seconds)
            .and_then(|_| {
                let path = path.to_str().ok_or("sshConfigUnsafe")?.replace('\\', "/");
                FrozenProfile::build(
                    self.plan.input.identity,
                    self.scope.host_key_sha256,
                    trust,
                    &path,
                )
            });
        match profile {
            Ok(profile) => {
                self.profile = Some((path.into(), profile));
                self.runtime.prepared(task);
                Ok(())
            }
            Err(error) => {
                self.stop(task, End::SetupFailed, cleanup_deadline());
                Err(error)
            }
        }
    }

    pub fn refresh(
        &mut self,
        task: &Task,
        current: WindowsBridgeInput<'_>,
        lease: SignedRecord<'_>,
        source: &mut impl StatusSource,
        mut clock: impl FnMut() -> ClockSample,
    ) -> bool {
        if !self.runtime.owns(task)
            || !matches!(
                self.runtime.phase,
                Phase::AwaitingAuthorization | Phase::AwaitingResources | Phase::Available(_)
            )
        {
            return false;
        }
        if !self.in_context(task, current, clock()) {
            return false;
        }
        self.policy
            .refresh(lease, &self.history, source, || Observation {
                scope: &self.scope,
                clock: clock(),
            });
        if matches!(self.policy.state(), State::Active(_)) {
            self.runtime.authorized(task);
            return true;
        }
        self.stop(task, self.policy_end(), cleanup_deadline());
        false
    }

    pub fn activate(
        &mut self,
        task: &Task,
        current: WindowsBridgeInput<'_>,
        mut clock: impl FnMut() -> ClockSample,
        start: impl FnOnce() -> io::Result<OwnedProcess>,
    ) -> bool {
        if !self.runtime.owns(task)
            || !matches!(self.runtime.phase, Phase::AwaitingResources)
            || !self.in_context(task, current, clock())
        {
            return false;
        }
        let Some((path, profile)) = self.profile.take() else {
            self.stop(task, End::SetupFailed, cleanup_deadline());
            return false;
        };
        if self.check_policy(&mut clock).is_none() {
            self.stop(task, self.policy_end(), cleanup_deadline());
            return false;
        }
        let mut cleanup = None;
        let files = match FrozenResources::create_with_cleanup_deadline(&path, profile, || {
            Some(*cleanup.get_or_insert_with(cleanup_deadline))
        }) {
            Ok(files) => files,
            Err(error) => {
                self.finish(
                    task,
                    End::SetupFailed,
                    cleanup.unwrap_or_else(cleanup_deadline),
                    LocalResources {
                        files: None,
                        process: None,
                        terminal: None,
                        terminal_close: None,
                        released_any: error.released_any,
                    },
                    error.pending,
                    Some(error.error),
                );
                return false;
            }
        };
        let mut resources = LocalResources {
            files: Some(files),
            process: None,
            terminal: None,
            terminal_close: None,
            released_any: false,
        };
        let setup = self.start_owned(
            &mut resources.files,
            &mut resources.process,
            &mut clock,
            start,
        );
        if let Err((end, error)) = setup {
            self.finish(task, end, cleanup_deadline(), resources, Vec::new(), error);
            return false;
        }
        self.runtime.ready(task, resources).is_ok()
    }

    fn start_owned(
        &mut self,
        files: &mut Option<FrozenResources>,
        slot: &mut Option<OwnedProcess>,
        clock: &mut impl FnMut() -> ClockSample,
        start: impl FnOnce() -> io::Result<OwnedProcess>,
    ) -> Result<(), (End, Option<io::Error>)> {
        let setup_error = |error| (End::SetupFailed, Some(error));
        let files = files
            .as_mut()
            .ok_or_else(|| setup_error(io::ErrorKind::NotFound.into()))?;
        files.validate().map_err(setup_error)?;
        if self.check_policy(clock).is_none() {
            return Err((self.policy_end(), None));
        }
        let child = start().map_err(setup_error)?;
        let child = slot.insert(child);
        files.validate().map_err(setup_error)?;
        if self.check_policy(clock).is_none() {
            return Err((self.policy_end(), None));
        }
        child.resume().map_err(setup_error)?;
        if self.check_policy(clock).is_none() {
            return Err((self.policy_end(), None));
        }
        Ok(())
    }

    pub fn available(&self, task: &Task, now: ClockSample) -> bool {
        self.runtime.owns(task)
            && matches!(&self.runtime.phase, Phase::Available(resources)
                if resources.process.as_ref().is_some_and(|process| matches!(process.is_running(), Ok(true))))
            && self.policy.active_at(now)
    }

    // This proof is for the offline lifecycle. LocalRuntime keeps the Job and
    // frozen file handles, and verification does not expose an upstream.
    pub fn begin_verification(
        &mut self,
        task: &Task,
        current: WindowsBridgeInput<'_>,
        args: &[String],
        clock: impl FnMut() -> ClockSample,
    ) -> BridgeResult<LocalVerification> {
        let (prepared, now) = self.verification_snapshot(task, current, clock)?;
        let checked_profile = prepared.profile.clone();
        match Verification::new(prepared, checked_profile.files(), args, (), now) {
            Ok(proof) => Ok(LocalVerification {
                task: task.clone(),
                proof,
            }),
            Err(error) => {
                self.stop(task, End::SetupFailed, cleanup_deadline());
                Err(error)
            }
        }
    }

    pub fn finish_verification(
        &mut self,
        attempt: LocalVerification,
        current: WindowsBridgeInput<'_>,
        args: &[String],
        clock: impl FnMut() -> ClockSample,
    ) -> BridgeResult<()> {
        let LocalVerification { task, proof } = attempt;
        let (prepared, now) = self.verification_snapshot(&task, current, clock)?;
        let result = proof.finish(&prepared, prepared.profile.files(), args, now);
        if result.is_err() {
            self.stop(&task, End::SetupFailed, cleanup_deadline());
        } else {
            self.transport_verified = true;
        }
        result
    }

    fn verification_snapshot(
        &mut self,
        task: &Task,
        current: WindowsBridgeInput<'_>,
        mut clock: impl FnMut() -> ClockSample,
    ) -> BridgeResult<(PreparedForward, ClockSample)> {
        if !self.runtime.owns(task) {
            return Err("windowsBridgeContextChanged".into());
        }
        let Phase::Available(resources) = &mut self.runtime.phase else {
            return Err("windowsForwardPolicyDenied".into());
        };
        if !resources
            .process
            .as_ref()
            .is_some_and(|process| matches!(process.is_running(), Ok(true)))
        {
            return Err("bridgeUnavailable".into());
        }
        let profile = match resources
            .files
            .as_mut()
            .ok_or(io::ErrorKind::NotFound.into())
            .and_then(FrozenResources::checked_profile)
        {
            Ok(profile) => profile.clone(),
            Err(_) => {
                self.stop(task, End::SetupFailed, cleanup_deadline());
                return Err("sshConfigUnsafe".into());
            }
        };
        let Some((policy, now)) = self.check_policy(&mut clock) else {
            self.stop(task, self.policy_end(), cleanup_deadline());
            return Err("windowsForwardPolicyDenied".into());
        };
        let policy = policy.clone();
        let forward = match self.plan.forward_args(current, now.unix_seconds) {
            Ok(forward) => forward,
            Err(error) => {
                self.stop(task, End::AuthorizationUnavailable, cleanup_deadline());
                return Err(error);
            }
        };
        Ok((
            PreparedForward::from_checked(self.plan, &forward, self.scope.clone(), profile, policy),
            now,
        ))
    }

    pub fn poll(
        &mut self,
        task: &Task,
        current: WindowsBridgeInput<'_>,
        mut clock: impl FnMut() -> ClockSample,
    ) -> bool {
        if !self.runtime.owns(task)
            || !matches!(self.runtime.phase, Phase::Available(_))
            || !self.in_context(task, current, clock())
        {
            return false;
        }
        if self.check_policy(&mut clock).is_none() {
            self.stop(task, self.policy_end(), cleanup_deadline());
            return false;
        }
        if matches!(&self.runtime.phase, Phase::Available(resources)
            if resources.process.as_ref().is_some_and(|process| !matches!(process.is_running(), Ok(true))))
        {
            self.stop(task, End::ProcessExited, cleanup_deadline());
            return false;
        }
        true
    }

    fn check_policy(
        &mut self,
        clock: &mut impl FnMut() -> ClockSample,
    ) -> Option<(&VerifiedPolicy, ClockSample)> {
        self.policy.check_current(&self.history, || Observation {
            scope: &self.scope,
            clock: clock(),
        })
    }

    fn in_context(
        &mut self,
        task: &Task,
        current: WindowsBridgeInput<'_>,
        now: ClockSample,
    ) -> bool {
        if self.plan.forward_args(current, now.unix_seconds).is_err() {
            self.stop(task, End::AuthorizationUnavailable, cleanup_deadline());
            return false;
        }
        true
    }

    fn policy_end(&self) -> End {
        match self.policy.state() {
            State::Expired => End::Expired,
            State::Revoked => End::Revoked,
            State::StorageFault => End::HistoryUnavailable,
            _ => End::AuthorizationUnavailable,
        }
    }

    pub fn stop(&mut self, task: &Task, end: End, deadline: Instant) -> Option<&Shutdown> {
        self.finish(
            task,
            end,
            deadline,
            LocalResources::default(),
            Vec::new(),
            None,
        )
    }

    fn finish(
        &mut self,
        task: &Task,
        end: End,
        deadline: Instant,
        staged: LocalResources,
        pending: Vec<PendingCleanup>,
        error: Option<io::Error>,
    ) -> Option<&Shutdown> {
        if !self.runtime.owns(task) {
            return None;
        }
        if matches!(
            self.runtime.phase,
            Phase::Closed(_) | Phase::ClosedAbnormally(_)
        ) {
            return self.runtime.close(task, |_| unreachable!("already closed"));
        }
        let deadline = if end == End::ApplicationExit {
            deadline.min(Instant::now() + COORDINATED_EXIT_BUDGET)
        } else {
            deadline
        };
        let ai = self
            .ai
            .close(if matches!(end, End::Cancelled | End::ApplicationExit) {
                WindowsAiState::Closed
            } else {
                WindowsAiState::Invalidated
            });
        self.transport_verified = false;
        self.policy.stop();
        self.profile = None;
        self.managed = ManagedPhase::Unprepared;
        self.runtime.close(task, |owned| {
            let mut report = owned.unwrap_or(staged).close(end, deadline, pending, error);
            report.ai = ai;
            if ai == WindowsAiState::CleanupIncomplete && report.completion == Completion::Complete
            {
                report.completion = Completion::Partial;
            }
            let abnormal = report.completion != Completion::Complete
                || !matches!(
                    end,
                    End::Cancelled | End::ApplicationExit | End::Expired | End::Revoked
                );
            (report, abnormal)
        })
    }
}

fn cleanup_deadline() -> Instant {
    Instant::now() + CLOSE_BUDGET
}

#[cfg(test)]
#[test]
fn runtime_boundaries_reports_no_progress_as_cleanup_failure() {
    let unresolved = PendingCleanup {
        name: ".",
        id: None,
        error: io::ErrorKind::PermissionDenied.into(),
    };
    let report = LocalResources::default().close(
        End::SetupFailed,
        cleanup_deadline(),
        vec![unresolved],
        None,
    );
    assert_eq!(report.completion, Completion::Failed);
    assert_eq!(report.pending.len(), 1);
    assert!(report.process.is_none());

    let unstopped = LocalResources::summarize(
        End::ApplicationExit,
        cleanup_deadline(),
        [
            Some(CloseReport {
                process_exited: false,
                job_empty: false,
                forced: true,
                termination_error: Some(5),
                timed_out: false,
            }),
            None,
        ],
        None,
        false,
        Vec::new(),
        None,
    );
    assert_eq!(unstopped.completion, Completion::Failed);

    let root_only = LocalResources::summarize(
        End::ApplicationExit,
        cleanup_deadline(),
        [
            Some(CloseReport {
                process_exited: true,
                job_empty: false,
                forced: true,
                termination_error: Some(5),
                timed_out: false,
            }),
            None,
        ],
        None,
        false,
        Vec::new(),
        None,
    );
    assert_eq!(root_only.completion, Completion::Partial);
}
