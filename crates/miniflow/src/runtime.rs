use std::sync::atomic::{AtomicUsize, Ordering};

static WORKER_COUNT: AtomicUsize = AtomicUsize::new(1);

/// Select the process-local Timely worker count used by generated `run()`
/// methods. Embedded callers that do not need process-wide configuration can
/// call a generated `run_with_workers` method directly.
#[doc(hidden)]
pub fn set_worker_count(workers: usize) {
    assert!(workers > 0, "MiniFlow requires at least one worker");
    WORKER_COUNT.store(workers, Ordering::Relaxed);
}

/// Return the configured process-local Timely worker count.
#[doc(hidden)]
#[must_use]
pub fn worker_count() -> usize {
    WORKER_COUNT.load(Ordering::Relaxed)
}
