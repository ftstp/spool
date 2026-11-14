//! The scan stays out of the way: its work runs on a pool of its own, on half the cores, in
//! Windows' background mode (lower CPU AND disk priority). The window and the
//! commands keep the processor and the disk first; on an idle machine the scan still gets
//! everything left. Seen on a 0.1.0 install: a MIDI pass on every core at normal priority
//! made a click wait 20 s.

use std::sync::LazyLock;

static POOL: LazyLock<rayon::ThreadPool> = LazyLock::new(|| {
    let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    rayon::ThreadPoolBuilder::new()
        // past ~16 threads the 100 MB sets fight over memory
        // half the cores: measured on 8 cores during a full scan (196,000 MIDI files), the
        // catalogue page took 1.8-2.5 s with a thread per core (they fight over memory, which
        // priority does not settle), 0.47 s with 4 threads (as with no scan), for a scan 20%
        // longer
        .num_threads((n / 2).clamp(1, 8))
        .thread_name(|i| format!("spool-scan-{i}"))
        .start_handler(|_| begin())
        .build()
        .expect("scan thread pool")
});

/// Runs `f` on the scan's pool: it and every parallel loop inside it (`par_iter`) are
/// background work.
pub fn run<R: Send>(f: impl FnOnce() -> R + Send) -> R {
    POOL.install(f)
}

/// While the scan holds the base, everyone waits for it: that stretch runs at normal
/// priority (a background thread holding the lock would make a click wait longer still).
pub struct Urgent(());

impl Urgent {
    pub fn now() -> Urgent {
        end();
        Urgent(())
    }
}

impl Drop for Urgent {
    fn drop(&mut self) {
        // back to the background, only on the scan's own threads
        if std::thread::current().name().is_some_and(|n| n.starts_with("spool-scan-")) {
            begin();
        }
    }
}

#[cfg(windows)]
fn end() {
    use windows_sys::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_MODE_BACKGROUND_END};
    // SAFETY: as in begin(); a thread not in background mode just gets an error, ignored
    unsafe {
        SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_END);
    }
}

#[cfg(not(windows))]
fn end() {}

#[cfg(windows)]
fn begin() {
    use windows_sys::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_MODE_BACKGROUND_BEGIN};
    // SAFETY: the pseudo-handle of the calling thread, always valid
    unsafe {
        SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN);
    }
}

#[cfg(not(windows))]
fn begin() {}

#[cfg(test)]
mod tests {
    #[test]
    fn work_runs_on_the_scan_pool() {
        use rayon::prelude::*;
        let names: Vec<String> = super::run(|| (0..64).into_par_iter().map(|_| std::thread::current().name().unwrap_or("").to_string()).collect());
        assert!(names.iter().all(|n| n.starts_with("spool-scan-")), "{names:?}");
    }
}
