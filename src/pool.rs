//! A tiny persistent thread pool.
//!
//! The solver calls [`Pool::run`] once per time step; spawning OS threads each step would cost
//! more than the step itself on small grids, so the workers are kept alive and woken through a
//! generation counter. `run(f)` calls `f(t)` once for every `t` in `0..threads()` (the calling
//! thread takes `t = 0`) and returns only when all calls have finished, which is what makes it
//! sound to hand the workers a reference to a closure living on the caller's stack.

use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

type Job = *const (dyn Fn(usize) + Sync);

struct JobPtr(Job);
// SAFETY: the pointee is `Sync`, and `run` keeps it alive until every worker is done with it.
unsafe impl Send for JobPtr {}

struct State {
    generation: u64,
    job: Option<JobPtr>,
    remaining: usize,
    shutdown: bool,
}

struct Inner {
    state: Mutex<State>,
    start: Condvar,
    done: Condvar,
}

pub struct Pool {
    threads: usize,
    inner: Arc<Inner>,
    handles: Vec<JoinHandle<()>>,
}

/// Blocks until all workers have finished the current job, also when the caller's own share
/// of the work panics (otherwise workers could outlive the closure they point to).
struct WaitGuard<'a>(&'a Inner);

impl Drop for WaitGuard<'_> {
    fn drop(&mut self) {
        let mut st = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        while st.remaining > 0 {
            st = self.0.done.wait(st).unwrap_or_else(|e| e.into_inner());
        }
        st.job = None;
    }
}

impl Pool {
    pub fn new(threads: usize) -> Pool {
        let threads = threads.max(1);
        let inner = Arc::new(Inner {
            state: Mutex::new(State { generation: 0, job: None, remaining: 0, shutdown: false }),
            start: Condvar::new(),
            done: Condvar::new(),
        });
        let handles = (1..threads)
            .map(|t| {
                let inner = Arc::clone(&inner);
                std::thread::spawn(move || worker(&inner, t))
            })
            .collect();
        Pool { threads, inner, handles }
    }

    pub fn threads(&self) -> usize {
        self.threads
    }

    pub fn run(&self, f: &(dyn Fn(usize) + Sync)) {
        if self.threads == 1 {
            f(0);
            return;
        }
        // SAFETY: only the lifetime is erased; `WaitGuard` below does not let this function
        // return (or unwind) before every worker has finished calling through the pointer.
        let job: Job = unsafe { std::mem::transmute(f) };
        {
            let mut st = self.inner.state.lock().unwrap();
            st.job = Some(JobPtr(job));
            st.generation += 1;
            st.remaining = self.threads - 1;
        }
        self.inner.start.notify_all();
        let _guard = WaitGuard(&self.inner);
        f(0);
    }
}

fn worker(inner: &Inner, index: usize) {
    let mut seen = 0u64;
    loop {
        let job = {
            let mut st = inner.state.lock().unwrap();
            while st.generation == seen && !st.shutdown {
                st = inner.start.wait(st).unwrap();
            }
            if st.shutdown {
                return;
            }
            seen = st.generation;
            st.job.as_ref().map(|j| j.0)
        };
        if let Some(job) = job {
            // SAFETY: see `Pool::run`. A panic in the job is caught so `remaining` still
            // reaches zero and the caller is not left waiting forever.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe { (*job)(index) }));
            let mut st = inner.state.lock().unwrap();
            st.remaining -= 1;
            if st.remaining == 0 {
                inner.done.notify_all();
            }
            drop(st);
            if result.is_err() {
                eprintln!("vortexlab: worker thread {index} panicked; aborting");
                std::process::abort();
            }
        }
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        self.inner.state.lock().unwrap_or_else(|e| e.into_inner()).shutdown = true;
        self.inner.start.notify_all();
        for h in self.handles.drain(..) {
            let _ = h.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn every_index_runs_exactly_once_per_call() {
        for threads in [1, 2, 3, 4] {
            let pool = Pool::new(threads);
            let hits: Vec<AtomicU64> = (0..threads).map(|_| AtomicU64::new(0)).collect();
            for _ in 0..500 {
                pool.run(&|t| {
                    hits[t].fetch_add(1, Ordering::Relaxed);
                });
            }
            for h in &hits {
                assert_eq!(h.load(Ordering::Relaxed), 500);
            }
        }
    }

    #[test]
    fn borrows_from_the_callers_stack() {
        let pool = Pool::new(4);
        let data: Vec<u64> = (0..1000).collect();
        let sums: Vec<AtomicU64> = (0..4).map(|_| AtomicU64::new(0)).collect();
        pool.run(&|t| {
            let s: u64 = data[t * 250..(t + 1) * 250].iter().sum();
            sums[t].store(s, Ordering::Relaxed);
        });
        let total: u64 = sums.iter().map(|s| s.load(Ordering::Relaxed)).sum();
        assert_eq!(total, 999 * 1000 / 2);
    }
}
