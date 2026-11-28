use super::{cleanup_deadline, CloseReport, End, LocalRuntime, Phase, Shutdown, Task};
use crate::features::{
    proxy::ProxyEndpoint,
    remote_bridge::target_os::windows::{process_environment::ProcessEnvironment, WindowsShell},
};
use std::{
    ffi::OsString,
    io,
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Component, Path, PathBuf, Prefix},
    time::Instant,
};

use crate::features::remote_bridge::target_os::windows_bridge::safety::{
    owned_process::OwnedProcess, policy::ClockSample, WindowsBridgeInput,
};

pub(super) enum ManagedPhase {
    Unprepared,
    AwaitingReceipt([u8; 16]),
    Prepared,
    Closed(CloseReport),
}

pub(in super::super::super) enum ManagedStatus<'a> {
    Unprepared,
    AwaitingReceipt,
    Prepared,
    TerminalClosed(CloseReport),
    Closed(&'a Shutdown),
}

pub(in super::super::super) struct ManagedLaunch<'a> {
    pub shell: WindowsShell,
    pub environment: &'a ProcessEnvironment,
    pub directory: &'a Path,
    pub receipt: &'a str,
}

impl LocalRuntime<'_, '_> {
    pub(in super::super::super) fn start_managed(
        &mut self,
        task: &Task,
        current: WindowsBridgeInput<'_>,
        environment: impl IntoIterator<Item = (OsString, OsString)>,
        directory: &Path,
        mut clock: impl FnMut() -> ClockSample,
        start: impl FnOnce(ManagedLaunch<'_>) -> io::Result<OwnedProcess>,
    ) -> bool {
        if !self.runtime.owns(task) || !matches!(self.managed, ManagedPhase::Unprepared) {
            return false;
        }
        let endpoint = ProxyEndpoint {
            host: "127.0.0.1".into(),
            port: self.plan.remote_port,
            protocol: self.plan.input.upstream.protocol,
        };
        let mut nonce = [0; 16];
        let shell = self.plan.input.shell;
        let launch = || {
            let directory = working_directory(directory)?;
            let environment = ProcessEnvironment::prepare(&endpoint, environment)
                .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
            getrandom::fill(&mut nonce)
                .map_err(|_| io::Error::other("managed receipt unavailable"))?;
            let receipt = receipt_for(nonce);
            start(ManagedLaunch {
                shell,
                environment: &environment,
                directory: &directory,
                receipt: &receipt,
            })
        };
        if matches!(self.runtime.phase, Phase::AwaitingResources) {
            // Retain the terminal-only offline fixture path from M3.3.
            if !self.activate(task, current, clock, launch) {
                return false;
            }
            if let Phase::Available(resources) = &mut self.runtime.phase {
                resources.terminal = resources.process.take();
            }
        } else {
            if !self.poll(task, current, &mut clock) {
                return false;
            }
            let Phase::Available(resources) = &mut self.runtime.phase else {
                return false;
            };
            let mut resources = std::mem::take(resources);
            let result = self.start_owned(
                &mut resources.files,
                &mut resources.terminal,
                &mut clock,
                launch,
            );
            if let Phase::Available(owned) = &mut self.runtime.phase {
                *owned = resources;
            }
            if let Err((end, error)) = result {
                self.finish(
                    task,
                    end,
                    cleanup_deadline(),
                    Default::default(),
                    Vec::new(),
                    error,
                );
                return false;
            }
        }
        self.managed = ManagedPhase::AwaitingReceipt(nonce);
        true
    }

    pub(in super::super::super) fn confirm_managed(
        &mut self,
        task: &Task,
        current: WindowsBridgeInput<'_>,
        receipt: &str,
        clock: impl FnMut() -> ClockSample,
    ) -> bool {
        let ManagedPhase::AwaitingReceipt(nonce) = self.managed else {
            return false;
        };
        if !self.poll_managed(task, current, clock) {
            return false;
        }
        if receipt != receipt_for(nonce) {
            self.stop_managed(task, End::SetupFailed, cleanup_deadline());
            return false;
        }
        self.managed = ManagedPhase::Prepared;
        true
    }

    pub(in super::super::super) fn poll_managed(
        &mut self,
        task: &Task,
        current: WindowsBridgeInput<'_>,
        clock: impl FnMut() -> ClockSample,
    ) -> bool {
        if !matches!(
            self.managed,
            ManagedPhase::AwaitingReceipt(_) | ManagedPhase::Prepared
        ) || !self.poll(task, current, clock)
        {
            return false;
        }
        let Phase::Available(resources) = &self.runtime.phase else {
            return false;
        };
        let end = match resources.terminal.as_ref().map(OwnedProcess::is_running) {
            Some(Ok(true)) => return true,
            Some(Ok(false)) => End::ProcessExited,
            _ => End::SetupFailed,
        };
        self.stop_managed(task, end, cleanup_deadline());
        false
    }

    pub(in super::super::super) fn stop_managed(
        &mut self,
        task: &Task,
        end: End,
        deadline: Instant,
    ) -> Option<CloseReport> {
        if !self.runtime.owns(task) {
            return None;
        }
        let Phase::Available(resources) = &mut self.runtime.phase else {
            return self
                .stop(task, end, deadline)
                .and_then(|report| report.terminal);
        };
        if resources.process.is_none() {
            return self
                .stop(task, end, deadline)
                .and_then(|report| report.terminal);
        }
        let report = resources.close_terminal(deadline)?;
        self.managed = ManagedPhase::Closed(report);
        Some(report)
    }

    // The owner polls this task when accepting a result or observing process exit.
    pub(in super::super::super) fn managed_status(&self, task: &Task) -> Option<ManagedStatus<'_>> {
        if !self.runtime.owns(task) {
            return None;
        }
        Some(match &self.runtime.phase {
            Phase::Closed(report) | Phase::ClosedAbnormally(report) => {
                ManagedStatus::Closed(report)
            }
            _ => match self.managed {
                ManagedPhase::Unprepared => ManagedStatus::Unprepared,
                ManagedPhase::AwaitingReceipt(_) => ManagedStatus::AwaitingReceipt,
                ManagedPhase::Prepared => ManagedStatus::Prepared,
                ManagedPhase::Closed(report) => ManagedStatus::TerminalClosed(report),
            },
        })
    }
}

fn receipt_for(nonce: [u8; 16]) -> String {
    format!("ROUTESPAN-MANAGED/1 {}", hex::encode(nonce))
}

fn working_directory(path: &Path) -> io::Result<PathBuf> {
    let local_absolute = |path: &Path| {
        path.is_absolute()
            && matches!(path.components().next(),
            Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
    };
    if !local_absolute(path) {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let resolved = path.canonicalize()?;
    if !local_absolute(&resolved) || !resolved.metadata()?.is_dir() {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let resolved: Vec<_> = resolved.as_os_str().encode_wide().collect();
    // cmd requires a conventional drive path for its current directory.
    let verbatim_prefix = b"\\\\?\\".map(u16::from);
    let drive_path = resolved.strip_prefix(&verbatim_prefix).unwrap_or(&resolved);
    Ok(PathBuf::from(OsString::from_wide(drive_path)))
}
