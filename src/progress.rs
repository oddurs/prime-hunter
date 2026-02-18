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
        eprintln!(
            "[{:02}:{:02}:{:02}] current: {} | tested: {} | {:.2}/s | primes found: {}",
            h, m, s, current, tested, rate, found
        );
    }

    pub fn stop(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_starts_at_zero() {
        let p = Progress::new();
        assert_eq!(p.tested.load(Ordering::Relaxed), 0);
        assert_eq!(p.found.load(Ordering::Relaxed), 0);
        assert_eq!(*p.current.lock().unwrap(), "");
    }

    #[test]
    fn increment_updates_value() {
        let p = Progress::new();
        p.tested.fetch_add(10, Ordering::Relaxed);
        p.found.fetch_add(3, Ordering::Relaxed);
        assert_eq!(p.tested.load(Ordering::Relaxed), 10);
        assert_eq!(p.found.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn current_string_updates() {
        let p = Progress::new();
        *p.current.lock().unwrap() = "42! (~51 digits)".to_string();
        assert_eq!(*p.current.lock().unwrap(), "42! (~51 digits)");
    }

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

    #[test]
    fn stop_sets_shutdown_flag() {
        let p = Progress::new();
        assert!(!p.shutdown.load(Ordering::Relaxed));
        p.stop();
        assert!(p.shutdown.load(Ordering::Relaxed));
    }

    #[test]
    fn print_status_does_not_panic() {
        let p = Progress::new();
        p.tested.fetch_add(100, Ordering::Relaxed);
        p.found.fetch_add(5, Ordering::Relaxed);
        *p.current.lock().unwrap() = "test".to_string();
        // Just verify it doesn't panic — output goes to stderr
        p.print_status();
    }
}
