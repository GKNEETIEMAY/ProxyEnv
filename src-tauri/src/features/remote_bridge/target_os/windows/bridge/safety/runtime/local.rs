use super::{Phase, Runtime, Task};
use crate::features::remote_bridge::{
    target_os::windows_bridge::safety::{
        frozen::{FrozenProfile, HostTrustEvidence},
        frozen_resources::{FrozenResources, PendingCleanup},
        owned_process::{CloseReport, OwnedProcess, PROCESS_WAIT_MS},
        plan_lease_scope,
        policy::{
            lifecycle::{Enrollment, Observation, PolicyLifecycle, State, StatusSource},
            ClockSample, LeaseScope, SignedRecord,
        },
        policy_storage::PolicyStorage,
        WindowsBridgeInput, WindowsBridgePlan,
    },
    BridgeResult,
};
use std::{
    io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const CLOSE_BUDGET: Duration = Duration::from_millis(PROCESS_WAIT_MS as u64);

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(in super::super) enum End {
    Cancelled,
    ApplicationExit,
    Expired,
    Revoked,
    AuthorizationUnavailable,
    HistoryUnavailable,
    SetupFailed,
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
    pub pending: Vec<PendingCleanup>,
    pub setup_error: Option<io::Error>,
}

#[derive(Default)]
struct LocalResources {
    files: Option<FrozenResources>,
    process: Option<OwnedProcess>,
    released_any: bool,
}

impl LocalResources {
    fn close(
        mut self,
        end: End,
        deadline: Instant,
        mut pending: Vec<PendingCleanup>,
        setup_error: Option<io::Error>,
    ) -> Shutdown {
        let process = self
            .process
            .take()
            .map(|mut process| process.close_until(deadline));
        let mut timed_out = process.is_some_and(|process| process.timed_out);
        let mut progress =
            self.released_any || process.is_some_and(|process| process.process_exited);
        if let Some(mut files) = self.files.take() {
            // The process must be confirmed stopped before its frozen inputs can be removed.
            let drained = process.is_none_or(|process| process.process_exited && process.job_empty);
            let file_deadline = if drained { deadline } else { Instant::now() };
            let (missing, released_any) = files.close_until(file_deadline);
            timed_out |= drained
                && (Instant::now() >= deadline
                    || missing
                        .iter()
                        .any(|entry| entry.error.kind() == io::ErrorKind::TimedOut));
            progress |= released_any;
            pending.extend(missing);
        }
        timed_out |= pending
            .iter()
            .any(|entry| entry.error.kind() == io::ErrorKind::TimedOut)
            && process.is_none_or(|process| process.process_exited && process.job_empty);
        let clean_process = process.is_none_or(|process| {
            process.process_exited && process.job_empty && process.termination_error.is_none()
        });
        let completion = if timed_out {
            Completion::TimedOut
        } else if clean_process && pending.is_empty() {
            Completion::Complete
        } else if progress {
            Completion::Partial
        } else {
            Completion::Failed
        };
        Shutdown {
            end,
            completion,
            process,
            pending,
            setup_error,
        }
    }
}

impl Drop for LocalResources {
    fn drop(&mut self) {
        if self.process.is_none() && self.files.is_none() {
            return;
        }
        let owned = Self {
            process: self.process.take(),
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
    runtime: Runtime<LocalResources, Shutdown>,
    plan: &'p WindowsBridgePlan<'review>,
    history: PolicyStorage,
    scope: LeaseScope,
    policy: PolicyLifecycle<'review>,
    profile: Option<(PathBuf, FrozenProfile)>,
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
                runtime,
                plan,
                history,
                scope,
                policy,
                profile: None,
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
        if !self.check_policy(&mut clock) {
            self.stop(task, self.policy_end(), cleanup_deadline());
            return false;
        }
        let files = match FrozenResources::create_with_cleanup_deadline(&path, profile, || {
            Some(cleanup_deadline())
        }) {
            Ok(files) => files,
            Err(error) => {
                self.finish(
                    task,
                    End::SetupFailed,
                    cleanup_deadline(),
                    LocalResources {
                        files: None,
                        process: None,
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
            released_any: false,
        };
        let setup = self.start_owned(&mut resources, &mut clock, start);
        if let Err((end, error)) = setup {
            self.finish(task, end, cleanup_deadline(), resources, Vec::new(), error);
            return false;
        }
        self.runtime.ready(task, resources).is_ok()
    }

    fn start_owned(
        &mut self,
        resources: &mut LocalResources,
        clock: &mut impl FnMut() -> ClockSample,
        start: impl FnOnce() -> io::Result<OwnedProcess>,
    ) -> Result<(), (End, Option<io::Error>)> {
        let setup_error = |error| (End::SetupFailed, Some(error));
        let files = resources
            .files
            .as_mut()
            .ok_or_else(|| setup_error(io::ErrorKind::NotFound.into()))?;
        files.validate().map_err(setup_error)?;
        if !self.check_policy(clock) {
            return Err((self.policy_end(), None));
        }
        let child = start().map_err(setup_error)?;
        let child = resources.process.insert(child);
        files.validate().map_err(setup_error)?;
        if !self.check_policy(clock) {
            return Err((self.policy_end(), None));
        }
        child.resume().map_err(setup_error)?;
        if !self.check_policy(clock) {
            return Err((self.policy_end(), None));
        }
        Ok(())
    }

    pub fn available(&self, task: &Task, now: ClockSample) -> bool {
        self.runtime.owns(task)
            && matches!(self.runtime.phase, Phase::Available(_))
            && self.policy.active_at(now)
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
        if self.check_policy(&mut clock) {
            return true;
        }
        self.stop(task, self.policy_end(), cleanup_deadline());
        false
    }

    fn check_policy(&mut self, clock: &mut impl FnMut() -> ClockSample) -> bool {
        self.policy.check_current(&self.history, || Observation {
            scope: &self.scope,
            clock: clock(),
        });
        matches!(self.policy.state(), State::Active(_))
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
        self.policy.stop();
        self.profile = None;
        self.runtime.close(task, |owned| {
            let report = owned.unwrap_or(staged).close(end, deadline, pending, error);
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
}
