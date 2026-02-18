//! # Events — Structured Event Bus for Search Activity
//!
//! A bounded, thread-safe event log that collects structured events from engine
//! search modules and transforms them into notifications for the frontend.
//!
//! ## Event Types
//!
//! | Variant | Emitted When |
//! |---------|-------------|
//! | `PrimeFound` | A new prime is discovered and logged to the database |
//! | `SearchStarted` | A search subprocess begins execution |
//! | `SearchCompleted` | A search finishes (with summary statistics) |
//! | `Milestone` | Notable progress (e.g., digit record, sieve phase complete) |
//! | `Warning` | Non-fatal issues (e.g., heartbeat timeout, checkpoint failure) |
//! | `Error` | Fatal errors that terminate a search |
//!
//! ## Delivery
//!
//! Events are stored in a `VecDeque` (bounded to prevent unbounded growth)
//! and converted to `Notification` structs for WebSocket delivery to the
//! Next.js frontend. Each notification gets a monotonic `id` for deduplication.

use serde::Serialize;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

/// Events emitted by search modules.
#[derive(Clone, Debug)]
pub enum Event {
    PrimeFound {
        form: String,
        expression: String,
        digits: u64,
        proof_method: String,
        timestamp: Instant,
    },
    SearchStarted {
        search_type: String,
        params: String,
        timestamp: Instant,
    },
    SearchCompleted {
        search_type: String,
        tested: u64,
        found: u64,
        elapsed_secs: f64,
        timestamp: Instant,
    },
    Milestone {
        message: String,
        timestamp: Instant,
    },
    Warning {
        context: String,
        message: String,
        timestamp: Instant,
    },
    Error {
        context: String,
        message: String,
        timestamp: Instant,
    },
}

/// A squashed notification ready for delivery to the frontend.
#[derive(Clone, Debug, Serialize)]
pub struct Notification {
    pub id: u64,
    pub kind: String,
    pub title: String,
    pub details: Vec<String>,
    pub count: u32,
    pub timestamp_ms: u64,
}

/// Central event bus: search modules emit events, the bus handles logging,
/// buffering, squashing, and broadcasting notifications via WebSocket.
pub struct EventBus {
    recent: Mutex<VecDeque<EventRecord>>,
    pending_primes: Mutex<Vec<PendingPrime>>,
    last_flush: Mutex<Instant>,
    notifications: Mutex<VecDeque<Notification>>,
    next_id: AtomicU64,
    ws_sender: Mutex<Option<tokio::sync::broadcast::Sender<String>>>,
    start: Instant,
}

#[derive(Clone, Debug, Serialize)]
pub struct EventRecord {
    pub kind: String,
    pub message: String,
    pub elapsed_secs: f64,
}

#[derive(Clone, Debug)]
struct PendingPrime {
    form: String,
    expression: String,
    digits: u64,
    proof_method: String,
}

fn elapsed_tag(start: Instant) -> String {
    let secs = start.elapsed().as_secs();
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

const RECENT_EVENTS_CAP: usize = 200;
const NOTIFICATIONS_CAP: usize = 50;
const FLUSH_INTERVAL_SECS: u64 = 10;

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    pub fn new() -> Self {
        EventBus {
            recent: Mutex::new(VecDeque::with_capacity(RECENT_EVENTS_CAP)),
            pending_primes: Mutex::new(Vec::new()),
            last_flush: Mutex::new(Instant::now()),
            notifications: Mutex::new(VecDeque::with_capacity(NOTIFICATIONS_CAP)),
            next_id: AtomicU64::new(1),
            ws_sender: Mutex::new(None),
            start: Instant::now(),
        }
    }

    /// Set the broadcast sender for WebSocket delivery.
    pub fn set_ws_sender(&self, sender: tokio::sync::broadcast::Sender<String>) {
        *self.ws_sender.lock().unwrap() = Some(sender);
    }

    /// Subscribe to notification broadcasts (one receiver per WS client).
    pub fn subscribe_ws(&self) -> tokio::sync::broadcast::Receiver<String> {
        self.ws_sender
            .lock()
            .unwrap()
            .as_ref()
            .expect("ws_sender not set")
            .subscribe()
    }

    /// Emit an event. Safe to call from rayon threads (no async).
    pub fn emit(&self, event: Event) {
        let elapsed = self.start.elapsed().as_secs_f64();
        let tag = elapsed_tag(self.start);

        match &event {
            Event::PrimeFound {
                form,
                expression,
                digits,
                proof_method,
                ..
            } => {
                eprintln!(
                    "[{}] PRIME {} {} {} digits {}",
                    tag, form, expression, digits, proof_method
                );
                self.push_record(
                    "prime",
                    &format!(
                        "{} {} ({} digits, {})",
                        form, expression, digits, proof_method
                    ),
                    elapsed,
                );
                self.pending_primes.lock().unwrap().push(PendingPrime {
                    form: form.clone(),
                    expression: expression.clone(),
                    digits: *digits,
                    proof_method: proof_method.clone(),
                });
            }
            Event::SearchStarted {
                search_type,
                params,
                ..
            } => {
                eprintln!("[{}] SEARCH_START {} {}", tag, search_type, params);
                self.push_record(
                    "search_start",
                    &format!("{} {}", search_type, params),
                    elapsed,
                );
                self.broadcast_notification(Notification {
                    id: self.next_id.fetch_add(1, Ordering::Relaxed),
                    kind: "search_start".into(),
                    title: format!("Search started: {}", search_type),
                    details: vec![params.clone()],
                    count: 1,
                    timestamp_ms: now_ms(),
                });
            }
            Event::SearchCompleted {
                search_type,
                tested,
                found,
                elapsed_secs,
                ..
            } => {
                eprintln!(
                    "[{}] SEARCH_DONE {} tested={} found={} elapsed={:.1}s",
                    tag, search_type, tested, found, elapsed_secs
                );
                self.push_record(
                    "search_done",
                    &format!("{} tested={} found={}", search_type, tested, found),
                    elapsed,
                );
                self.broadcast_notification(Notification {
                    id: self.next_id.fetch_add(1, Ordering::Relaxed),
                    kind: "search_done".into(),
                    title: format!("Search complete: {}", search_type),
                    details: vec![format!(
                        "Tested {} candidates, found {} primes in {:.1}s",
                        tested, found, elapsed_secs
                    )],
                    count: 1,
                    timestamp_ms: now_ms(),
                });
            }
            Event::Milestone { message, .. } => {
                eprintln!("[{}] MILESTONE {}", tag, message);
                self.push_record("milestone", message, elapsed);
                self.broadcast_notification(Notification {
                    id: self.next_id.fetch_add(1, Ordering::Relaxed),
                    kind: "milestone".into(),
                    title: message.clone(),
                    details: vec![],
                    count: 1,
                    timestamp_ms: now_ms(),
                });
            }
            Event::Warning {
                context, message, ..
            } => {
                eprintln!("[{}] WARN [{}] {}", tag, context, message);
                self.push_record("warning", &format!("[{}] {}", context, message), elapsed);
            }
            Event::Error {
                context, message, ..
            } => {
                eprintln!("[{}] ERROR [{}] {}", tag, context, message);
                self.push_record("error", &format!("[{}] {}", context, message), elapsed);
                self.broadcast_notification(Notification {
                    id: self.next_id.fetch_add(1, Ordering::Relaxed),
                    kind: "error".into(),
                    title: format!("Error: {}", context),
                    details: vec![message.clone()],
                    count: 1,
                    timestamp_ms: now_ms(),
                });
            }
        }

        // Auto-flush pending primes if enough time has passed
        let should_flush = {
            let last = self.last_flush.lock().unwrap();
            last.elapsed().as_secs() >= FLUSH_INTERVAL_SECS
        };
        if should_flush {
            self.flush();
        }
    }

    /// Flush pending primes: squash by form and broadcast as notifications.
    pub fn flush(&self) {
        let primes: Vec<PendingPrime> = {
            let mut pending = self.pending_primes.lock().unwrap();
            std::mem::take(&mut *pending)
        };
        *self.last_flush.lock().unwrap() = Instant::now();

        if primes.is_empty() {
            return;
        }

        // Group by form
        let mut groups: std::collections::HashMap<String, Vec<PendingPrime>> =
            std::collections::HashMap::new();
        for p in primes {
            groups.entry(p.form.clone()).or_default().push(p);
        }

        for (form, items) in &groups {
            let count = items.len() as u32;
            let title = if count == 1 {
                format!("{} prime found", form)
            } else {
                format!("{} {} primes found", count, form)
            };

            let max_details = 5;
            let mut details: Vec<String> = items
                .iter()
                .take(max_details)
                .map(|p| format!("{} ({} digits, {})", p.expression, p.digits, p.proof_method))
                .collect();
            if items.len() > max_details {
                details.push(format!("and {} more", items.len() - max_details));
            }

            self.broadcast_notification(Notification {
                id: self.next_id.fetch_add(1, Ordering::Relaxed),
                kind: "prime".into(),
                title,
                details,
                count,
                timestamp_ms: now_ms(),
            });
        }
    }

    /// Get recent notifications for new WS connections.
    pub fn recent_notifications(&self, limit: usize) -> Vec<Notification> {
        let notifs = self.notifications.lock().unwrap();
        notifs.iter().rev().take(limit).cloned().collect()
    }

    /// Get recent events for the API.
    pub fn recent_events(&self, limit: usize) -> Vec<EventRecord> {
        let events = self.recent.lock().unwrap();
        events.iter().rev().take(limit).cloned().collect()
    }

    fn push_record(&self, kind: &str, message: &str, elapsed: f64) {
        let mut recent = self.recent.lock().unwrap();
        if recent.len() >= RECENT_EVENTS_CAP {
            recent.pop_front();
        }
        recent.push_back(EventRecord {
            kind: kind.into(),
            message: message.into(),
            elapsed_secs: elapsed,
        });
    }

    fn broadcast_notification(&self, notification: Notification) {
        {
            let mut notifs = self.notifications.lock().unwrap();
            if notifs.len() >= NOTIFICATIONS_CAP {
                notifs.pop_front();
            }
            notifs.push_back(notification.clone());
        }
        if let Some(sender) = self.ws_sender.lock().unwrap().as_ref() {
            let json = serde_json::json!({
                "type": "notification",
                "notification": notification,
            });
            let _ = sender.send(json.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bus() -> EventBus {
        EventBus::new()
    }

    fn prime_event(form: &str, expr: &str) -> Event {
        Event::PrimeFound {
            form: form.into(),
            expression: expr.into(),
            digits: 10,
            proof_method: "deterministic".into(),
            timestamp: Instant::now(),
        }
    }

    #[test]
    fn new_event_bus_has_no_events() {
        let bus = make_bus();
        assert!(bus.recent_events(100).is_empty());
        assert!(bus.recent_notifications(100).is_empty());
    }

    #[test]
    fn emit_prime_found_recorded_in_events() {
        let bus = make_bus();
        bus.emit(prime_event("factorial", "3!+1"));
        let events = bus.recent_events(100);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, "prime");
        assert!(events[0].message.contains("factorial"));
        assert!(events[0].message.contains("3!+1"));
    }

    #[test]
    fn emit_prime_found_does_not_create_immediate_notification() {
        // Primes are batched in pending_primes, only flushed on timer or manual flush
        let bus = make_bus();
        bus.emit(prime_event("factorial", "3!+1"));
        // No notification yet (primes are batched)
        assert!(bus.recent_notifications(100).is_empty());
    }

    #[test]
    fn emit_search_started_creates_notification() {
        let bus = make_bus();
        bus.emit(Event::SearchStarted {
            search_type: "kbn".into(),
            params: "k=3 b=2".into(),
            timestamp: Instant::now(),
        });
        let events = bus.recent_events(100);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, "search_start");

        let notifs = bus.recent_notifications(100);
        assert_eq!(notifs.len(), 1);
        assert_eq!(notifs[0].kind, "search_start");
        assert!(notifs[0].title.contains("kbn"));
    }

    #[test]
    fn emit_search_completed_creates_notification() {
        let bus = make_bus();
        bus.emit(Event::SearchCompleted {
            search_type: "factorial".into(),
            tested: 1000,
            found: 5,
            elapsed_secs: 3.14,
            timestamp: Instant::now(),
        });
        let notifs = bus.recent_notifications(100);
        assert_eq!(notifs.len(), 1);
        assert_eq!(notifs[0].kind, "search_done");
        assert!(notifs[0].details[0].contains("1000"));
        assert!(notifs[0].details[0].contains("5"));
    }

    #[test]
    fn emit_milestone_creates_notification() {
        let bus = make_bus();
        bus.emit(Event::Milestone {
            message: "Reached 10000 digits".into(),
            timestamp: Instant::now(),
        });
        let notifs = bus.recent_notifications(100);
        assert_eq!(notifs.len(), 1);
        assert_eq!(notifs[0].kind, "milestone");
        assert_eq!(notifs[0].title, "Reached 10000 digits");
    }

    #[test]
    fn emit_warning_no_notification() {
        let bus = make_bus();
        bus.emit(Event::Warning {
            context: "sieve".into(),
            message: "low memory".into(),
            timestamp: Instant::now(),
        });
        let events = bus.recent_events(100);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, "warning");

        // Warnings do NOT create notifications
        assert!(bus.recent_notifications(100).is_empty());
    }

    #[test]
    fn emit_error_creates_notification() {
        let bus = make_bus();
        bus.emit(Event::Error {
            context: "db".into(),
            message: "connection lost".into(),
            timestamp: Instant::now(),
        });
        let notifs = bus.recent_notifications(100);
        assert_eq!(notifs.len(), 1);
        assert_eq!(notifs[0].kind, "error");
        assert!(notifs[0].title.contains("db"));
    }

    #[test]
    fn flush_squashes_primes_by_form() {
        let bus = make_bus();
        // Emit primes of two different forms
        bus.emit(prime_event("factorial", "3!+1"));
        bus.emit(prime_event("factorial", "11!+1"));
        bus.emit(prime_event("kbn", "2^31-1"));

        bus.flush();

        let notifs = bus.recent_notifications(100);
        assert_eq!(notifs.len(), 2); // one per form

        let factorial_notif = notifs.iter().find(|n| n.title.contains("factorial")).unwrap();
        assert_eq!(factorial_notif.count, 2);

        let kbn_notif = notifs.iter().find(|n| n.title.contains("kbn")).unwrap();
        assert_eq!(kbn_notif.count, 1);
    }

    #[test]
    fn flush_empty_is_noop() {
        let bus = make_bus();
        bus.flush();
        assert!(bus.recent_notifications(100).is_empty());
    }

    #[test]
    fn flush_clears_pending_primes() {
        let bus = make_bus();
        bus.emit(prime_event("factorial", "3!+1"));
        bus.flush();
        // Second flush should not produce more notifications
        let count_after_first = bus.recent_notifications(100).len();
        bus.flush();
        assert_eq!(bus.recent_notifications(100).len(), count_after_first);
    }

    #[test]
    fn flush_caps_details_at_five() {
        let bus = make_bus();
        for i in 0..8 {
            bus.emit(prime_event("factorial", &format!("{}!+1", i)));
        }
        bus.flush();

        let notifs = bus.recent_notifications(100);
        assert_eq!(notifs.len(), 1);
        // 5 detail lines + "and 3 more" = 6
        assert_eq!(notifs[0].details.len(), 6);
        assert!(notifs[0].details[5].contains("and 3 more"));
    }

    #[test]
    fn recent_events_capped_at_200() {
        let bus = make_bus();
        // Emit 250 warnings (warnings add to events but not notifications)
        for i in 0..250 {
            bus.emit(Event::Warning {
                context: "test".into(),
                message: format!("msg {}", i),
                timestamp: Instant::now(),
            });
        }
        let events = bus.recent_events(300);
        assert_eq!(events.len(), RECENT_EVENTS_CAP); // capped at 200
    }

    #[test]
    fn recent_notifications_capped_at_50() {
        let bus = make_bus();
        // Emit 60 milestones (each creates a notification)
        for i in 0..60 {
            bus.emit(Event::Milestone {
                message: format!("milestone {}", i),
                timestamp: Instant::now(),
            });
        }
        let notifs = bus.recent_notifications(100);
        assert_eq!(notifs.len(), NOTIFICATIONS_CAP); // capped at 50
    }

    #[test]
    fn recent_events_returns_most_recent_first() {
        let bus = make_bus();
        bus.emit(Event::Warning {
            context: "a".into(),
            message: "first".into(),
            timestamp: Instant::now(),
        });
        bus.emit(Event::Warning {
            context: "b".into(),
            message: "second".into(),
            timestamp: Instant::now(),
        });
        let events = bus.recent_events(10);
        assert_eq!(events.len(), 2);
        assert!(events[0].message.contains("second")); // most recent first
        assert!(events[0].elapsed_secs >= events[1].elapsed_secs);
    }

    #[test]
    fn notification_ids_are_unique_and_increasing() {
        let bus = make_bus();
        bus.emit(Event::Milestone {
            message: "a".into(),
            timestamp: Instant::now(),
        });
        bus.emit(Event::Milestone {
            message: "b".into(),
            timestamp: Instant::now(),
        });
        let notifs = bus.recent_notifications(10);
        // Most recent first, so notifs[0].id > notifs[1].id
        assert!(notifs[0].id > notifs[1].id);
    }
}
