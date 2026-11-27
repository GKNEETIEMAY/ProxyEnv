use std::sync::Arc;

#[cfg(windows)]
pub(super) mod local;

#[derive(Clone)]
pub(super) struct Task(Arc<()>);

enum Phase<R, C> {
    Unprepared,
    Preparing,
    AwaitingAuthorization,
    AwaitingResources,
    Available(R),
    Closing,
    Closed(C),
    ClosedAbnormally(C),
}

struct Runtime<R, C> {
    owner: Option<Task>,
    phase: Phase<R, C>,
}

impl<R, C> Default for Runtime<R, C> {
    fn default() -> Self {
        Self {
            owner: None,
            phase: Phase::Unprepared,
        }
    }
}

impl<R, C> Runtime<R, C> {
    fn begin(&mut self) -> Option<Task> {
        if !matches!(
            self.phase,
            Phase::Unprepared | Phase::Closed(_) | Phase::ClosedAbnormally(_)
        ) {
            return None;
        }
        let task = Task(Arc::new(()));
        self.owner = Some(task.clone());
        self.phase = Phase::Preparing;
        Some(task)
    }

    fn owns(&self, task: &Task) -> bool {
        self.owner
            .as_ref()
            .is_some_and(|owner| Arc::ptr_eq(&owner.0, &task.0))
    }

    fn prepared(&mut self, task: &Task) -> bool {
        if !self.owns(task) || !matches!(self.phase, Phase::Preparing) {
            return false;
        }
        self.phase = Phase::AwaitingAuthorization;
        true
    }

    // Called after the existing lifecycle has confirmed the grant and persisted state.
    fn authorized(&mut self, task: &Task) -> bool {
        if !self.owns(task) || !matches!(self.phase, Phase::AwaitingAuthorization) {
            return false;
        }
        self.phase = Phase::AwaitingResources;
        true
    }

    fn ready(&mut self, task: &Task, resources: R) -> Result<(), R> {
        if !self.owns(task) || !matches!(self.phase, Phase::AwaitingResources) {
            return Err(resources);
        }
        self.phase = Phase::Available(resources);
        Ok(())
    }

    fn close(&mut self, task: &Task, cleanup: impl FnOnce(Option<R>) -> (C, bool)) -> Option<&C> {
        if !self.owns(task) || matches!(self.phase, Phase::Unprepared | Phase::Closing) {
            return None;
        }
        if !matches!(self.phase, Phase::Closed(_) | Phase::ClosedAbnormally(_)) {
            let resources = match std::mem::replace(&mut self.phase, Phase::Closing) {
                Phase::Available(resources) => Some(resources),
                _ => None,
            };
            let (report, abnormal) = cleanup(resources);
            self.phase = if abnormal {
                Phase::ClosedAbnormally(report)
            } else {
                Phase::Closed(report)
            };
        }
        match &self.phase {
            Phase::Closed(report) | Phase::ClosedAbnormally(report) => Some(report),
            _ => None,
        }
    }
}

#[cfg(test)]
mod ownership_tests {
    use super::*;

    #[test]
    fn lifecycle_ownership_rejects_resources_before_authorization() {
        let mut runtime = Runtime::<&str, usize>::default();
        assert!(matches!(runtime.phase, Phase::Unprepared));
        let task = runtime.begin().unwrap();
        assert!(matches!(runtime.phase, Phase::Preparing));
        assert!(runtime.begin().is_none());
        assert!(!runtime.authorized(&task));
        assert_eq!(runtime.ready(&task, "early"), Err("early"));
        assert!(runtime.prepared(&task));
        assert!(matches!(runtime.phase, Phase::AwaitingAuthorization));
        assert!(!runtime.prepared(&task));
        assert_eq!(runtime.ready(&task, "unapproved"), Err("unapproved"));
        assert!(runtime.authorized(&task));
        assert!(matches!(runtime.phase, Phase::AwaitingResources));
        assert!(runtime.ready(&task, "owned").is_ok());
        assert!(matches!(runtime.phase, Phase::Available("owned")));
        assert_eq!(
            runtime.close(&task, |resources| (resources.unwrap().len(), false)),
            Some(&5)
        );
        assert!(matches!(runtime.phase, Phase::Closed(5)));
        assert_eq!(runtime.close(&task, |_| panic!("repeat cleanup")), Some(&5));
    }

    #[test]
    fn lifecycle_ownership_ignores_obsolete_and_foreign_tasks() {
        let mut runtime = Runtime::<u32, u32>::default();
        let mut other = Runtime::<u32, u32>::default();
        let foreign = other.begin().unwrap();
        let first = runtime.begin().unwrap();
        assert_eq!(
            runtime.close(&first, |resources| {
                assert!(resources.is_none());
                (1, true)
            }),
            Some(&1)
        );
        assert!(matches!(runtime.phase, Phase::ClosedAbnormally(1)));

        let current = runtime.begin().unwrap();
        for stale in [&first, &foreign] {
            assert!(!runtime.prepared(stale));
            assert!(!runtime.authorized(stale));
            assert_eq!(runtime.ready(stale, 7), Err(7));
            assert_eq!(runtime.close(stale, |_| panic!("stale cleanup")), None);
        }
        assert!(runtime.prepared(&current));
        assert!(runtime.authorized(&current));
        assert!(runtime.ready(&current, 9).is_ok());
        assert_eq!(
            runtime.close(&current, |resources| (resources.unwrap(), false)),
            Some(&9)
        );
    }
}
