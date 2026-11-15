//! Live console output of running download/install jobs, one per tracked repository.
//!
//! Worker threads append lines here while `gh` / `flatpak` run; the UI polls a snapshot.
//! (UI signals can't be written from worker threads, so this is a plain mutex-guarded map.)

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Instant;

/// Keep at most this many lines per job (flatpak progress output can be long).
const MAX_LINES: usize = 2000;

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub lines: Vec<String>,
    pub running: bool,
    /// Bytes downloaded / total, while a download is running.
    pub progress: Option<(u64, u64)>,
    /// Seconds since the last new line (to explain long silent steps).
    pub quiet_secs: u64,
    /// Changes whenever anything above changes.
    pub generation: u64,
}

struct Job {
    lines: Vec<String>,
    running: bool,
    progress: Option<(u64, u64)>,
    last_line: Instant,
    generation: u64,
    cancel: Arc<AtomicBool>,
}

static JOBS: LazyLock<Mutex<HashMap<u64, Job>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn with_job<T>(id: u64, f: impl FnOnce(&mut Job) -> T) -> Option<T> {
    let mut jobs = JOBS.lock().unwrap_or_else(|e| e.into_inner());
    jobs.get_mut(&id).map(f)
}

/// Handle for a running job: append output, report progress, check for cancellation.
#[derive(Clone)]
pub struct Handle {
    id: u64,
    cancel: Arc<AtomicBool>,
}

impl Handle {
    /// A handle that isn't shown anywhere (tests, background use).
    pub fn detached() -> Handle {
        Handle { id: u64::MAX, cancel: Arc::new(AtomicBool::new(false)) }
    }

    pub fn line(&self, text: impl Into<String>) {
        let text = text.into();
        with_job(self.id, |j| {
            j.lines.push(text);
            if j.lines.len() > MAX_LINES {
                let extra = j.lines.len() - MAX_LINES;
                j.lines.drain(..extra);
            }
            j.last_line = Instant::now();
            j.generation += 1;
        });
    }

    /// Overwrite the last line (a progress bar redrawing itself).
    pub fn replace_last(&self, text: impl Into<String>) {
        let text = text.into();
        with_job(self.id, |j| {
            match j.lines.last_mut() {
                Some(l) => *l = text,
                None => j.lines.push(text),
            }
            j.last_line = Instant::now();
            j.generation += 1;
        });
    }

    pub fn progress(&self, p: Option<(u64, u64)>) {
        with_job(self.id, |j| {
            if j.progress != p {
                j.progress = p;
                j.generation += 1;
            }
        });
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        self.cancel.clone()
    }
}

/// Start (or restart) the job for a repository, clearing its previous output.
pub fn start(id: u64) -> Handle {
    let cancel = Arc::new(AtomicBool::new(false));
    let mut jobs = JOBS.lock().unwrap_or_else(|e| e.into_inner());
    let generation = jobs.get(&id).map_or(0, |j| j.generation + 1);
    jobs.insert(
        id,
        Job { lines: Vec::new(), running: true, progress: None, last_line: Instant::now(), generation, cancel: cancel.clone() },
    );
    Handle { id, cancel }
}

pub fn finish(id: u64) {
    with_job(id, |j| {
        j.running = false;
        j.progress = None;
        j.generation += 1;
    });
}

/// Ask a running job to stop (the running command gets SIGTERM).
pub fn cancel(id: u64) {
    with_job(id, |j| j.cancel.store(true, Ordering::Relaxed));
}

pub fn snapshot(id: u64) -> Option<Snapshot> {
    let jobs = JOBS.lock().unwrap_or_else(|e| e.into_inner());
    jobs.get(&id).map(|j| Snapshot {
        lines: j.lines.clone(),
        running: j.running,
        progress: j.progress,
        quiet_secs: j.last_line.elapsed().as_secs(),
        generation: j.generation,
    })
}

/// Just the generation counter, so pollers can skip unchanged output cheaply.
pub fn generation(id: u64) -> Option<(u64, bool)> {
    let jobs = JOBS.lock().unwrap_or_else(|e| e.into_inner());
    jobs.get(&id).map(|j| (j.generation, j.running))
}

/// The whole output as one string (for the activity log).
pub fn text(id: u64) -> String {
    snapshot(id).map(|s| s.lines.join("\n")).unwrap_or_default()
}

pub fn clear(id: u64) {
    JOBS.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
}
