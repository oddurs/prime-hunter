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
