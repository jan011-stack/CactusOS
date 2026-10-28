//! Cooperative fixed-slot scheduler using triangular-wave phase slices.

use crate::hyper_math::hyper_sin;
use crate::hyper_resource_manager::{
    allocate_linear, AllocationError, AllocationSummary, ResourceRequest,
};
use crate::hyper_shield::{HyperShieldController, RootCapability, SecurityRing};

pub const TASK_SLOTS: usize = 64;
pub const TASK_SIDE: usize = 8;
pub type TaskFn = fn();

#[derive(Clone, Copy)]
struct ScheduledTask {
    callback: TaskFn,
    ring: SecurityRing,
    info: TaskInfo,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TaskInfo {
    pub pid: u8,
    pub slot: u8,
    pub name: &'static str,
    pub weight: u32,
    pub coordinate: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulerError {
    SlotOutOfRange,
    SlotOccupied,
    EmptyTaskName,
}

/// Single-owner scheduler. Store it in static memory and call through `&mut`.
/// Tasks must be short, non-blocking callbacks; this scheduler creates no threads.
pub struct HyperScheduler {
    tasks: [Option<ScheduledTask>; TASK_SLOTS],
    execution_mask: u64,
}

impl HyperScheduler {
    pub const fn new() -> Self {
        Self {
            tasks: [None; TASK_SLOTS],
            execution_mask: 0,
        }
    }

    pub fn register(&mut self, slot: usize, task: TaskFn) -> Result<(), SchedulerError> {
        self.register_named(slot, "task", task)
    }

    pub fn register_named(
        &mut self,
        slot: usize,
        name: &'static str,
        task: TaskFn,
    ) -> Result<(), SchedulerError> {
        self.register_named_in_ring(slot, name, task, SecurityRing::Ring1)
    }

    pub fn register_in_ring(
        &mut self,
        slot: usize,
        task: TaskFn,
        ring: SecurityRing,
    ) -> Result<(), SchedulerError> {
        self.register_named_in_ring(slot, "task", task, ring)
    }

    pub fn register_named_in_ring(
        &mut self,
        slot: usize,
        name: &'static str,
        task: TaskFn,
        ring: SecurityRing,
    ) -> Result<(), SchedulerError> {
        if slot >= TASK_SLOTS {
            return Err(SchedulerError::SlotOutOfRange);
        }
        if name.is_empty() {
            return Err(SchedulerError::EmptyTaskName);
        }
        let bit = 1u64 << slot;
        if self.execution_mask & bit != 0 {
            return Err(SchedulerError::SlotOccupied);
        }
        let weight = name
            .as_bytes()
            .iter()
            .fold(0u32, |total, byte| total.saturating_add(byte.count_ones()));
        self.tasks[slot] = Some(ScheduledTask {
            callback: task,
            ring,
            info: TaskInfo {
                pid: (slot + 1) as u8,
                slot: slot as u8,
                name,
                weight,
                coordinate: weight,
            },
        });
        self.execution_mask |= bit;
        Ok(())
    }

    pub fn unregister(&mut self, slot: usize) -> Result<(), SchedulerError> {
        if slot >= TASK_SLOTS {
            return Err(SchedulerError::SlotOutOfRange);
        }
        self.execution_mask &= !(1u64 << slot);
        self.tasks[slot] = None;
        Ok(())
    }

    /// Execute one 8-slot row selected by the triangular-wave phase.
    /// Executes Ring 1 only. `phase` uses the period-8 domain.
    pub fn run_slice(&self, phase: f64) -> usize {
        self.dispatch(phase, None)
    }

    /// Dispatch selected tasks through the ring controller. A denied Ring 0
    /// callback purges `protected_memory` and panics before the callback runs.
    pub fn run_slice_guarded(
        &self,
        phase: f64,
        shield: &HyperShieldController,
        capability: Option<&RootCapability>,
        protected_memory: &mut [u8],
    ) -> usize {
        self.dispatch(phase, Some((shield, capability, protected_memory)))
    }

    fn dispatch(
        &self,
        phase: f64,
        guard: Option<(
            &HyperShieldController,
            Option<&RootCapability>,
            &mut [u8],
        )>,
    ) -> usize {
        let normalized = (hyper_sin(phase) + 1.0) * 4.0;
        let row = (normalized as usize).min(TASK_SIDE - 1);
        let row_mask = 0xffu64 << (row * TASK_SIDE);
        let mut pending = self.execution_mask & row_mask;
        let mut executed = 0;

        let mut guard = guard;
        while pending != 0 {
            let slot = pending.trailing_zeros() as usize;
            pending &= pending - 1;
            if let Some(task) = self.tasks[slot] {
                match (task.ring, guard.as_mut()) {
                    (SecurityRing::Ring1, _) => (task.callback)(),
                    (SecurityRing::Ring0, Some((shield, capability, memory))) => {
                        shield.enforce_or_panic(
                            SecurityRing::Ring0,
                            *capability,
                            memory,
                        );
                        (task.callback)();
                    }
                    (SecurityRing::Ring0, None) => continue,
                }
                executed += 1;
            }
        }
        executed
    }

    pub const fn execution_mask(&self) -> u64 {
        self.execution_mask
    }

    pub fn task_info(&self, slot: usize) -> Option<TaskInfo> {
        self.tasks
            .get(slot)
            .copied()
            .flatten()
            .map(|task| task.info)
    }

    pub const fn task_count(&self) -> usize {
        self.execution_mask.count_ones() as usize
    }

    /// Calculate quotas for registered tasks; inactive slots receive no quota.
    /// Requests are explicit and indexed by scheduler slot, so task names do
    /// not implicitly determine resource priority.
    pub fn resource_quotas(
        &self,
        requests: &[ResourceRequest; TASK_SLOTS],
        budget: u64,
        cursor: usize,
        allocations: &mut [u64; TASK_SLOTS],
    ) -> Result<AllocationSummary, AllocationError> {
        let inactive = ResourceRequest {
            minimum: 0,
            demand: 0,
            maximum: 0,
        };
        let mut active_requests = [inactive; TASK_SLOTS];
        for (slot, task) in self.tasks.iter().enumerate() {
            if task.is_some() {
                active_requests[slot] = requests[slot];
            }
        }
        allocate_linear(&active_requests, budget, cursor, allocations)
    }
}

impl Default for HyperScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{HyperScheduler, SchedulerError, TaskInfo, TASK_SLOTS};
    use crate::hyper_resource_manager::ResourceRequest;
    use crate::hyper_shield::{HyperShieldController, SecurityRing};
    use crate::terminal::{HyperSpaceTerminal, ROOT_TOKEN_HASH};
    use core::sync::atomic::{AtomicUsize, Ordering};
    use std::panic::{catch_unwind, AssertUnwindSafe};

    static RUNS: AtomicUsize = AtomicUsize::new(0);
    static NAMED_RUNS: AtomicUsize = AtomicUsize::new(0);
    static PROTECTED_RUNS: AtomicUsize = AtomicUsize::new(0);

    fn count_run() {
        RUNS.fetch_add(1, Ordering::Relaxed);
    }

    fn count_protected_run() {
        PROTECTED_RUNS.fetch_add(1, Ordering::Relaxed);
    }

    fn count_named_run() {
        NAMED_RUNS.fetch_add(1, Ordering::Relaxed);
    }

    #[test]
    fn phase_selects_only_the_matching_orthogonal_row() {
        RUNS.store(0, Ordering::Relaxed);
        let mut scheduler = HyperScheduler::new();
        scheduler.register(56, count_run).unwrap();
        scheduler.register(57, count_run).unwrap();
        scheduler.register(0, count_run).unwrap();

        assert_eq!(scheduler.run_slice(2.0), 2);
        assert_eq!(RUNS.load(Ordering::Relaxed), 2);
        assert_eq!(scheduler.run_slice(6.0), 1);
        assert_eq!(RUNS.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn mask_tracks_registration_and_rejects_invalid_slots() {
        let mut scheduler = HyperScheduler::new();
        scheduler.register(63, count_run).unwrap();
        assert_eq!(scheduler.execution_mask(), 1u64 << 63);
        assert_eq!(scheduler.register(63, count_run), Err(SchedulerError::SlotOccupied));
        assert_eq!(scheduler.register(TASK_SLOTS, count_run), Err(SchedulerError::SlotOutOfRange));
        scheduler.unregister(63).unwrap();
        assert_eq!(scheduler.execution_mask(), 0);
    }

    #[test]
    fn named_tasks_expose_stable_metadata_without_allocations() {
        NAMED_RUNS.store(0, Ordering::Relaxed);
        let mut scheduler = HyperScheduler::new();
        scheduler
            .register_named(0, "INIT", count_named_run)
            .unwrap();
        scheduler
            .register_named(1, "RUN_HYPER", count_named_run)
            .unwrap();

        assert_eq!(scheduler.task_count(), 2);
        assert_eq!(
            scheduler.task_info(0),
            Some(TaskInfo {
                pid: 1,
                slot: 0,
                name: "INIT",
                weight: 13,
                coordinate: 13,
            })
        );
        assert_eq!(
            scheduler.task_info(1).map(|task| task.name),
            Some("RUN_HYPER")
        );
        assert_eq!(scheduler.task_info(TASK_SLOTS), None);
        assert_eq!(
            scheduler.register_named(2, "", count_run),
            Err(SchedulerError::EmptyTaskName)
        );
        assert_eq!(scheduler.run_slice(6.0), 2);
        assert_eq!(NAMED_RUNS.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn resource_quotas_use_explicit_demands_and_ignore_inactive_slots() {
        let mut scheduler = HyperScheduler::new();
        scheduler.register(2, count_run).unwrap();
        scheduler.register(7, count_run).unwrap();

        let inactive = ResourceRequest {
            minimum: 0,
            demand: 0,
            maximum: 0,
        };
        let mut requests = [inactive; TASK_SLOTS];
        requests[2] = ResourceRequest {
            minimum: 0,
            demand: 0,
            maximum: 10,
        };
        requests[7] = ResourceRequest {
            minimum: 0,
            demand: 1,
            maximum: 10,
        };
        let mut allocations = [u64::MAX; TASK_SLOTS];

        let summary = scheduler
            .resource_quotas(&requests, 10, 0, &mut allocations)
            .unwrap();

        assert_eq!(allocations[2], 3);
        assert_eq!(allocations[7], 7);
        assert!(allocations
            .iter()
            .enumerate()
            .all(|(slot, allocation)| slot == 2 || slot == 7 || *allocation == 0));
        assert_eq!(summary.allocated, 10);
        assert_eq!(summary.unused, 0);
    }

    #[test]
    fn ring_zero_callback_requires_capability_and_purges_on_denial() {
        PROTECTED_RUNS.store(0, Ordering::Relaxed);
        let mut scheduler = HyperScheduler::new();
        scheduler
            .register_in_ring(56, count_protected_run, SecurityRing::Ring0)
            .unwrap();
        assert_eq!(scheduler.run_slice(2.0), 0);

        let shield = HyperShieldController::new();
        let mut protected_memory = [0xa5; 8];
        let denied = catch_unwind(AssertUnwindSafe(|| {
            scheduler.run_slice_guarded(2.0, &shield, None, &mut protected_memory)
        }));
        assert!(denied.is_err());
        assert_eq!(protected_memory, [0; 8]);
        assert_eq!(PROTECTED_RUNS.load(Ordering::Relaxed), 0);

        let terminal = HyperSpaceTerminal::new();
        let mut active_shield = HyperShieldController::new();
        let capability = active_shield
            .activate_root(&terminal, &ROOT_TOKEN_HASH)
            .unwrap();
        assert_eq!(
            scheduler.run_slice_guarded(
                2.0,
                &active_shield,
                Some(&capability),
                &mut protected_memory,
            ),
            1
        );
        assert_eq!(PROTECTED_RUNS.load(Ordering::Relaxed), 1);
    }
}