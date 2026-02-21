//! # Progress — Atomic Search Progress Counters
//!
//! Thread-safe progress tracking shared between engine search threads and the
//! background status reporter. Uses atomics for lock-free counter updates
//! from parallel Rayon workers, and a Mutex only for the current-candidate
//! string (low contention — updated once per block, not per candidate).
//!
//! ## Background Reporter
//!
//! A dedicated thread prints progress to stderr every 30 seconds:
//! tested count, found count, rate (candidates/sec), and current candidate.
//! Shuts down cleanly via the `shutdown` atomic flag.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tracing::info;

pub struct Progress {
    pub tested: AtomicU64,
    pub found: AtomicU64,
    pub current: Mutex<String>,
    start: Instant,
    shutdown: AtomicBool,
}

impl Progress {
    pub fn new() -> Arc<Self> {
        Arc::new(Progress {
            tested: AtomicU64::new(0),
            found: AtomicU64::new(0),
            current: Mutex::new(String::new()),
            start: Instant::now(),
            shutdown: AtomicBool::new(false),
        })
    }

    pub fn start_reporter(self: &Arc<Self>) -> thread::JoinHandle<()> {
        let progress = Arc::clone(self);
        thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(30));
            if progress.shutdown.load(Ordering::Relaxed) {
                break;
            }
            progress.print_status();
        })
    }

    pub fn print_status(&self) {
        let elapsed = self.start.elapsed();
        let tested = self.tested.load(Ordering::Relaxed);
        let found = self.found.load(Ordering::Relaxed);
        let current = self.current.lock().unwrap().clone();
        let rate = if elapsed.as_secs() > 0 {
            tested as f64 / elapsed.as_secs_f64()
        } else {
            0.0
        };
        let h = elapsed.as_secs() / 3600;
        let m = (elapsed.as_secs() % 3600) / 60;
        let s = elapsed.as_secs() % 60;
        info!(
            current = %current,
            tested,
            rate = format_args!("{:.2}", rate),
            found,
            elapsed = format_args!("{:02}:{:02}:{:02}", h, m, s),
            "search progress"
        );
    }

    pub fn stop(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    //! Tests for atomic search progress counters and background reporter.
    //!
    //! Validates initialization to zero, single-threaded increments, concurrent
    //! multi-threaded increments (atomicity guarantees), shutdown signal
    //! propagation across threads, and print_status safety including the
    //! zero-elapsed-time edge case.
    //!
    //! ## Thread-Safety Model
    //!
    //! Progress uses AtomicU64 for `tested` and `found` (lock-free, updated by
    //! rayon workers), Mutex<String> for `current` (low contention, updated once
    //! per block), and AtomicBool for `shutdown` (cross-thread signal). The
    //! concurrent tests verify that 8 threads performing 1000 increments each
    //! always yield exactly 8000, confirming atomic correctness.

    use super::*;

    // ── Initialization ──────────────────────────────────────────────

    /// All counters must start at zero and the current string must be empty.
    /// This is the initial state before any search work begins.
    #[test]
    fn counter_starts_at_zero() {
        let p = Progress::new();
        assert_eq!(p.tested.load(Ordering::Relaxed), 0);
        assert_eq!(p.found.load(Ordering::Relaxed), 0);
        assert_eq!(*p.current.lock().unwrap(), "");
    }

    // ── Single-Threaded Increments ─────────────────────────────────

    /// Basic fetch_add on tested and found counters. In production, the
    /// engine thread increments `tested` for each candidate and `found`
    /// for each discovered prime.
    #[test]
    fn increment_updates_value() {
        let p = Progress::new();
        p.tested.fetch_add(10, Ordering::Relaxed);
        p.found.fetch_add(3, Ordering::Relaxed);
        assert_eq!(p.tested.load(Ordering::Relaxed), 10);
        assert_eq!(p.found.load(Ordering::Relaxed), 3);
    }

    /// The current-candidate string is updated once per block (not per
    /// candidate) to show what the engine is working on. Low contention:
    /// only one writer (engine) and one reader (reporter thread).
    #[test]
    fn current_string_updates() {
        let p = Progress::new();
        *p.current.lock().unwrap() = "42! (~51 digits)".to_string();
        assert_eq!(*p.current.lock().unwrap(), "42! (~51 digits)");
    }

    // ── Concurrent Increment Correctness ────────────────────────────

    /// 8 threads each increment `tested` by 1 a total of 1000 times. The
    /// final value must be exactly 8000, proving that AtomicU64::fetch_add
    /// with Relaxed ordering is sufficient for monotonic counters — no
    /// increments are lost even under heavy contention.
    #[test]
    fn concurrent_increments_are_accurate() {
        let p = Progress::new();
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let p = Arc::clone(&p);
                thread::spawn(move || {
                    for _ in 0..1000 {
                        p.tested.fetch_add(1, Ordering::Relaxed);
                    }
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        assert_eq!(p.tested.load(Ordering::Relaxed), 8000);
    }

    // ── Shutdown Signal ────────────────────────────────────────────

    /// The stop() method sets the shutdown flag, causing the background
    /// reporter thread to exit on its next 30-second wake cycle.
    #[test]
    fn stop_sets_shutdown_flag() {
        let p = Progress::new();
        assert!(!p.shutdown.load(Ordering::Relaxed));
        p.stop();
        assert!(p.shutdown.load(Ordering::Relaxed));
    }

    // ── Status Printing ────────────────────────────────────────────

    /// print_status must not panic under any state. Output goes to stderr
    /// in the format: [HH:MM:SS] current: ... | tested: N | rate/s | primes found: N
    #[test]
    fn print_status_does_not_panic() {
        let p = Progress::new();
        p.tested.fetch_add(100, Ordering::Relaxed);
        p.found.fetch_add(5, Ordering::Relaxed);
        *p.current.lock().unwrap() = "test".to_string();
        // Just verify it doesn't panic — output goes to stderr
        p.print_status();
    }

    // ── Additional Concurrent Increment Tests ──────────────────────

    /// Same as concurrent_increments_are_accurate but for the `found` counter.
    /// 4 threads x 500 increments = 2000 total. Primes are rarer than candidates
    /// tested, so fewer threads contend on this counter in practice.
    #[test]
    fn concurrent_found_increments_are_accurate() {
        let p = Progress::new();
        let threads: Vec<_> = (0..4)
            .map(|_| {
                let p = Arc::clone(&p);
                thread::spawn(move || {
                    for _ in 0..500 {
                        p.found.fetch_add(1, Ordering::Relaxed);
                    }
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        assert_eq!(p.found.load(Ordering::Relaxed), 2000);
    }

    /// Verifies that `tested` and `found` are independent counters — all
    /// 4 threads increment `tested`, but only thread 0 increments `found`.
    /// This mirrors production: every rayon worker increments tested, but
    /// only the thread that discovers a prime increments found.
    #[test]
    fn concurrent_tested_and_found_independent() {
        let p = Progress::new();
        let handles: Vec<_> = (0..4)
            .map(|i| {
                let p = Arc::clone(&p);
                thread::spawn(move || {
                    for _ in 0..1000 {
                        p.tested.fetch_add(1, Ordering::Relaxed);
                        if i == 0 {
                            // Only thread 0 finds primes
                            p.found.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(p.tested.load(Ordering::Relaxed), 4000);
        assert_eq!(p.found.load(Ordering::Relaxed), 1000);
    }

    /// Concurrent Mutex<String> writes from 4 threads. The final value is
    /// non-deterministic (last writer wins), but it must be a valid value
    /// from one of the threads — no corruption or partial writes.
    #[test]
    fn concurrent_current_string_updates() {
        let p = Progress::new();
        let handles: Vec<_> = (0..4)
            .map(|i| {
                let p = Arc::clone(&p);
                thread::spawn(move || {
                    for j in 0..100 {
                        *p.current.lock().unwrap() = format!("thread-{}-item-{}", i, j);
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        // The current string should be one of the valid values (last writer wins)
        let current = p.current.lock().unwrap().clone();
        assert!(current.starts_with("thread-"), "current should be a valid thread value, got: {}", current);
    }

    /// Large batch increments (millions) must accumulate correctly. Some
    /// search forms test millions of candidates per block and call fetch_add
    /// with a batch count rather than incrementing per-candidate.
    #[test]
    fn large_increment_values() {
        let p = Progress::new();
        p.tested.fetch_add(1_000_000, Ordering::Relaxed);
        p.tested.fetch_add(2_000_000, Ordering::Relaxed);
        assert_eq!(p.tested.load(Ordering::Relaxed), 3_000_000);
    }

    /// The shutdown flag must be visible across threads. A background thread
    /// polls the flag in a tight loop; the main thread calls stop(). The
    /// background thread must observe the change and exit. This validates
    /// cross-thread visibility of AtomicBool with Relaxed ordering.
    #[test]
    fn stop_is_visible_across_threads() {
        let p = Progress::new();
        let p2 = Arc::clone(&p);
        let handle = thread::spawn(move || {
            // Wait until shutdown is signaled
            while !p2.shutdown.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(1));
            }
            true
        });
        // Give the thread a moment to start
        thread::sleep(Duration::from_millis(10));
        p.stop();
        let result = handle.join().unwrap();
        assert!(result, "Thread should have observed shutdown signal");
    }

    /// Verifies that the reporter's shutdown flag is set correctly without
    /// actually starting the reporter thread (which sleeps 30s). In production,
    /// stop() is called during graceful shutdown, and the reporter exits on
    /// its next wake cycle.
    #[test]
    fn reporter_stops_cleanly() {
        let p = Progress::new();
        p.tested.fetch_add(42, Ordering::Relaxed);
        // Stop immediately so the reporter loop exits on its next check
        p.stop();
        // The reporter sleeps 30s before checking, so we just verify
        // the flag is set and don't actually start the reporter thread
        assert!(p.shutdown.load(Ordering::Relaxed));
    }

    // ── Edge Cases ────────────────────────────────────────────────

    /// Immediately after creation, elapsed time is ~0 seconds. The rate
    /// calculation must handle this without division by zero — it returns
    /// 0.0 when elapsed.as_secs() == 0.
    #[test]
    fn print_status_with_zero_elapsed() {
        // Immediately after creation, elapsed is ~0s — rate should be 0.0
        let p = Progress::new();
        // Should not divide by zero or panic
        p.print_status();
    }

    /// Multiple calls to stop() must be idempotent — storing true to an
    /// already-true AtomicBool is a no-op. This can happen if both the
    /// coordinator stop signal and the search completion trigger shutdown.
    #[test]
    fn multiple_stops_are_idempotent() {
        let p = Progress::new();
        p.stop();
        p.stop();
        p.stop();
        assert!(p.shutdown.load(Ordering::Relaxed));
    }
}
