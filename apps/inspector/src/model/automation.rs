use std::collections::BinaryHeap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex, OnceLock};

/// Priority-queue worker pool for all automation calls used by the inspector.
///
/// UIA uses COM MTA (`COINIT_MULTITHREADED`), so IUIAutomationElement objects can
/// be freely used across MTA threads.  Each pool thread initialises its own
/// thread-local COM/UIA singleton on first use.  A shared priority queue ensures
/// interactive tasks (Reveal, Highlight) are always processed before background
/// tasks (InitialLoad, Search), even when background threads are busy.
type Job = Box<dyn FnOnce() + Send + 'static>;

/// Number of threads in the automation worker pool.
const POOL_SIZE: usize = 3;

/// Task types tracked by the worker pool.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum TaskKind {
    /// Initial tree load from desktop root.
    InitialLoad,
    /// XPath search evaluation.
    Search,
    /// Tree reveal (ancestor preload).
    Reveal,
    /// Node highlighting.
    Highlight,
}

impl TaskKind {
    /// Scheduling priority: higher value → processed first.
    fn priority(self) -> u8 {
        match self {
            TaskKind::Reveal => 30,     // user selected a node — low latency required
            TaskKind::Highlight => 20,  // visual feedback
            TaskKind::Search => 10,     // user-triggered but not latency-critical
            TaskKind::InitialLoad => 0, // background bulk load
        }
    }
}

// ── Priority queue ────────────────────────────────────────────────────────────

struct PriorityJob {
    priority: u8,
    /// Monotonically increasing sequence number — used for FIFO ordering within
    /// the same priority level.
    seq: u64,
    job: Job,
}

impl PartialEq for PriorityJob {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.seq == other.seq
    }
}
impl Eq for PriorityJob {}

impl PartialOrd for PriorityJob {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PriorityJob {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // BinaryHeap is a max-heap: the "greatest" element is popped first.
        // Higher priority value → pop first.
        // Equal priority: lower sequence number → pop first (FIFO).
        self.priority.cmp(&other.priority).then_with(|| other.seq.cmp(&self.seq))
    }
}

struct SharedQueue {
    /// (heap, shutdown_flag)
    inner: Mutex<(BinaryHeap<PriorityJob>, bool)>,
    condvar: Condvar,
    next_seq: AtomicU64,
}

impl SharedQueue {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new((BinaryHeap::new(), false)),
            condvar: Condvar::new(),
            next_seq: AtomicU64::new(0),
        })
    }

    fn push(&self, priority: u8, job: Job) {
        let seq = self.next_seq.fetch_add(1, Ordering::Relaxed);
        let mut guard = self.inner.lock().expect("queue lock poisoned");
        guard.0.push(PriorityJob { priority, seq, job });
        self.condvar.notify_one();
    }

    /// Blocks the calling thread until a job is available or the queue is shut down.
    fn pop_blocking(&self) -> Option<Job> {
        let mut guard = self.inner.lock().expect("queue lock poisoned");
        loop {
            if let Some(pj) = guard.0.pop() {
                return Some(pj.job);
            }
            if guard.1 {
                return None; // shutdown
            }
            guard = self.condvar.wait(guard).expect("condvar wait failed");
        }
    }
}

// ── Worker pool ───────────────────────────────────────────────────────────────

struct AutomationWorkerPool {
    queue: Arc<SharedQueue>,
    /// Thread IDs of all pool threads — used by `run()` to detect re-entrant calls.
    worker_thread_ids: Vec<std::thread::ThreadId>,
}

fn pool() -> &'static AutomationWorkerPool {
    static POOL: OnceLock<AutomationWorkerPool> = OnceLock::new();
    POOL.get_or_init(|| {
        let queue = SharedQueue::new();
        let (id_tx, id_rx) = mpsc::sync_channel::<std::thread::ThreadId>(POOL_SIZE);

        for i in 0..POOL_SIZE {
            let queue_clone = Arc::clone(&queue);
            let id_tx_clone = id_tx.clone();
            std::thread::Builder::new()
                .name(format!("inspector-automation-worker-{i}"))
                .spawn(move || {
                    let _ = id_tx_clone.send(std::thread::current().id());
                    while let Some(job) = queue_clone.pop_blocking() {
                        job();
                    }
                })
                .expect("failed to spawn inspector-automation-worker");
        }
        drop(id_tx); // close sender side so id_rx terminates after POOL_SIZE receives

        let worker_thread_ids: Vec<_> =
            (0..POOL_SIZE).map(|_| id_rx.recv().expect("failed to receive worker thread id")).collect();

        tracing::debug!(
            pool_size = POOL_SIZE,
            thread_ids = ?worker_thread_ids,
            "automation worker pool started",
        );

        AutomationWorkerPool { queue, worker_thread_ids }
    })
}

static UI_THREAD_ID: OnceLock<std::thread::ThreadId> = OnceLock::new();

/// Global task counter for each task kind.
static ACTIVE_TASKS: &[(&str, &AtomicUsize)] = &[
    ("InitialLoad", &ACTIVE_INIT_LOAD),
    ("Search", &ACTIVE_SEARCH),
    ("Reveal", &ACTIVE_REVEAL),
    ("Highlight", &ACTIVE_HIGHLIGHT),
];

static ACTIVE_INIT_LOAD: AtomicUsize = AtomicUsize::new(0);
static ACTIVE_SEARCH: AtomicUsize = AtomicUsize::new(0);
static ACTIVE_REVEAL: AtomicUsize = AtomicUsize::new(0);
static ACTIVE_HIGHLIGHT: AtomicUsize = AtomicUsize::new(0);

/// Register the current thread as the inspector UI thread.
///
/// Should be called exactly once from the egui app initialization closure.
pub fn register_ui_thread() {
    let current = std::thread::current().id();
    match UI_THREAD_ID.set(current) {
        Ok(()) => {
            tracing::debug!(
                ui_thread = ?current,
                    pool_size = POOL_SIZE,
                "registered inspector UI thread",
            );
        }
        Err(existing) => {
            if existing != current {
                tracing::warn!(
                    registered_ui_thread = ?existing,
                    current_thread = ?current,
                    "register_ui_thread called from a different thread",
                );
            }
        }
    }
}

/// Run a closure on a pool worker and block the calling thread until it completes.
///
/// If called from within a pool worker thread (re-entrant), the closure is
/// executed directly to avoid deadlocks.
///
/// The job is dispatched with the highest possible priority so it is not starved
/// behind queued background tasks.
pub fn run<R, F>(f: F) -> R
where
    R: Send + 'static,
    F: FnOnce() -> R + Send + 'static,
{
    let p = pool();
    if p.worker_thread_ids.contains(&std::thread::current().id()) {
        return f();
    }

    let (result_tx, result_rx) = mpsc::sync_channel::<R>(1);
    p.queue.push(
        u8::MAX,
        Box::new(move || {
            let _ = result_tx.send(f());
        }),
    );
    result_rx.recv().expect("automation worker did not return result")
}

/// Dispatch a task to the pool with tracking: increments the per-kind counter on
/// start and decrements it on finish.  The job is scheduled according to the
/// task's priority so interactive tasks pre-empt pending background tasks.
pub fn dispatch_tracked<F>(kind: TaskKind, f: F)
where
    F: FnOnce() + Send + 'static,
{
    let counter = match kind {
        TaskKind::InitialLoad => &ACTIVE_INIT_LOAD,
        TaskKind::Search => &ACTIVE_SEARCH,
        TaskKind::Reveal => &ACTIVE_REVEAL,
        TaskKind::Highlight => &ACTIVE_HIGHLIGHT,
    };
    counter.fetch_add(1, Ordering::SeqCst);
    pool().queue.push(
        kind.priority(),
        Box::new(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
            counter.fetch_sub(1, Ordering::SeqCst);
            if let Err(e) = result {
                tracing::error!(task = ?kind, "task panicked");
                std::panic::resume_unwind(e);
            }
        }),
    );
}

/// Get a snapshot of all active tasks and their counts.
pub fn active_task_counts() -> Vec<(&'static str, usize)> {
    ACTIVE_TASKS.iter().map(|(name, counter)| (*name, counter.load(Ordering::SeqCst))).collect()
}

/// Get human-readable status of all active tasks.
#[allow(dead_code)]
pub fn worker_status() -> String {
    let counts = active_task_counts();
    let active: Vec<_> = counts.iter().filter(|(_, count)| *count > 0).collect();

    if active.is_empty() {
        "Idle".to_string()
    } else {
        let tasks: Vec<String> = active
            .iter()
            .map(|(name, count)| if *count > 1 { format!("{}({})", name, count) } else { name.to_string() })
            .collect();
        format!("Running: {}", tasks.join(", "))
    }
}
