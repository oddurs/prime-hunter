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
use tracing::{error, info, warn};

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
    next_event_id: AtomicU64,
    ws_sender: Mutex<Option<tokio::sync::broadcast::Sender<String>>>,
    start: Instant,
}

#[derive(Clone, Debug, Serialize)]
pub struct EventRecord {
    pub id: u64,
    pub kind: String,
    pub message: String,
    pub elapsed_secs: f64,
    pub timestamp_ms: u64,
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
            next_event_id: AtomicU64::new(1),
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
                // Truncate long expressions to prevent log bloat. Large primes
                // (e.g., 100K+ digit palindromics) can produce multi-KB expressions
                // that flood structured log aggregators (Loki, CloudWatch).
                let log_expr = if expression.len() > 1000 {
                    format!("{}...(truncated)", &expression[..1000])
                } else {
                    expression.clone()
                };
                info!(
                    form = %form,
                    expression = %log_expr,
                    digits,
                    proof_method = %proof_method,
                    elapsed = %tag,
                    "prime found"
                );
                self.push_record(
                    "prime",
                    &format!(
                        "{} {} ({} digits, {})",
                        form, expression, digits, proof_method
                    ),
                    elapsed,
                );
                // Broadcast individual prime_found event for real-time subscriptions.
                // Replaces Supabase Realtime INSERT subscription on the primes table.
                self.broadcast_prime_found(form, expression, *digits, proof_method);
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
                info!(search_type = %search_type, params = %params, elapsed = %tag, "search started");
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
                info!(
                    search_type = %search_type,
                    tested,
                    found,
                    elapsed_secs,
                    elapsed = %tag,
                    "search completed"
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
                info!(elapsed = %tag, "{}", message);
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
                warn!(context = %context, elapsed = %tag, "{}", message);
                self.push_record("warning", &format!("[{}] {}", context, message), elapsed);
            }
            Event::Error {
                context, message, ..
            } => {
                error!(context = %context, elapsed = %tag, "{}", message);
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

    /// Get events with id greater than `last_id`, in ascending order.
    pub fn recent_events_since(&self, last_id: u64, limit: usize) -> Vec<EventRecord> {
        let events = self.recent.lock().unwrap();
        let mut filtered: Vec<EventRecord> =
            events.iter().filter(|e| e.id > last_id).cloned().collect();
        filtered.sort_by_key(|e| e.id);
        if filtered.len() > limit {
            filtered.split_off(filtered.len() - limit)
        } else {
            filtered
        }
    }

    fn push_record(&self, kind: &str, message: &str, elapsed: f64) {
        let mut recent = self.recent.lock().unwrap();
        if recent.len() >= RECENT_EVENTS_CAP {
            recent.pop_front();
        }
        let id = self.next_event_id.fetch_add(1, Ordering::Relaxed);
        let timestamp_ms = now_ms();
        recent.push_back(EventRecord {
            id,
            kind: kind.into(),
            message: message.into(),
            elapsed_secs: elapsed,
            timestamp_ms,
        });
    }

    /// Broadcast an individual `prime_found` event via WebSocket.
    /// Replaces Supabase Realtime `postgres_changes` INSERT subscription
    /// on the `primes` table. The frontend's `use-prime-realtime` hook
    /// listens for messages with `type: "prime_found"`.
    fn broadcast_prime_found(&self, form: &str, expression: &str, digits: u64, proof_method: &str) {
        if let Some(sender) = self.ws_sender.lock().unwrap().as_ref() {
            let json = serde_json::json!({
                "type": "prime_found",
                "prime": {
                    "form": form,
                    "expression": expression,
                    "digits": digits,
                    "proof_method": proof_method,
                    "timestamp_ms": now_ms(),
                },
            });
            let _ = sender.send(json.to_string());
        }
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
    //! Tests for the structured event bus and notification delivery system.
    //!
    //! Validates event emission (all 6 variants), notification creation rules
    //! (which events create immediate notifications vs. batched), the prime
    //! batching/flush/squash pipeline, bounded buffer caps (200 events, 50
    //! notifications), ordering guarantees, WebSocket broadcast delivery,
    //! concurrent emit safety, and the `recent_events_since` polling API.
    //!
    //! ## Notification Rules
    //!
    //! | Event type | Immediate notification? | Batched? |
    //! |------------|------------------------|----------|
    //! | PrimeFound | No | Yes (flush every 10s) |
    //! | SearchStarted | Yes | No |
    //! | SearchCompleted | Yes | No |
    //! | Milestone | Yes | No |
    //! | Warning | No | No |
    //! | Error | Yes | No |
    //!
    //! ## Prime Batching
    //!
    //! PrimeFound events are collected in `pending_primes` and flushed
    //! periodically (every FLUSH_INTERVAL_SECS=10). On flush, primes are
    //! squashed by form: 5 factorial primes become one notification with
    //! count=5 and up to 5 detail lines (plus "and N more" if >5).

    use super::*;

    /// Helper: create a fresh EventBus for testing.
    fn make_bus() -> EventBus {
        EventBus::new()
    }

    /// Helper: create a PrimeFound event with default digit count and proof method.
    fn prime_event(form: &str, expr: &str) -> Event {
        Event::PrimeFound {
            form: form.into(),
            expression: expr.into(),
            digits: 10,
            proof_method: "deterministic".into(),
            timestamp: Instant::now(),
        }
    }

    // ── Initialization ──────────────────────────────────────────────

    /// A fresh EventBus must have empty event and notification buffers.
    #[test]
    fn new_event_bus_has_no_events() {
        let bus = make_bus();
        assert!(bus.recent_events(100).is_empty());
        assert!(bus.recent_notifications(100).is_empty());
    }

    // ── Event Emission ─────────────────────────────────────────────

    /// PrimeFound events are recorded in the event log with kind="prime"
    /// and a message containing form, expression, digits, and proof method.
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

    /// PrimeFound events are batched — they go into pending_primes, NOT
    /// into the notification queue. This prevents notification spam when
    /// many small primes are found in quick succession (e.g., factorial
    /// search at low n values).
    #[test]
    fn emit_prime_found_does_not_create_immediate_notification() {
        // Primes are batched in pending_primes, only flushed on timer or manual flush
        let bus = make_bus();
        bus.emit(prime_event("factorial", "3!+1"));
        // No notification yet (primes are batched)
        assert!(bus.recent_notifications(100).is_empty());
    }

    /// SearchStarted events create an immediate notification (not batched).
    /// The notification kind is "search_start" and the title includes the
    /// search type. Params are included in the details array.
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

    /// SearchCompleted events create an immediate notification with summary
    /// statistics: total tested, total found, and elapsed time. The elapsed
    /// time is formatted to 1 decimal place (e.g., "123.5s").
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

    /// Milestone events create immediate notifications. Used for notable
    /// progress like "Reached 10000 digits" or "Sieve phase complete".
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

    /// Warning events are logged to the event buffer but do NOT create
    /// notifications. Warnings are for operational issues (heartbeat timeout,
    /// low memory) that don't warrant user-facing alerts.
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

    /// Error events create immediate notifications. Errors represent fatal
    /// issues (DB connection lost, search abort) that require user attention.
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

    // ── Prime Batching and Flush ────────────────────────────────────

    /// Flush squashes pending primes by form: 2 factorial + 1 kbn primes
    /// produce 2 notifications (one per form). The factorial notification
    /// has count=2; the kbn notification has count=1. This prevents
    /// notification flooding during productive search runs.
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

        let factorial_notif = notifs
            .iter()
            .find(|n| n.title.contains("factorial"))
            .unwrap();
        assert_eq!(factorial_notif.count, 2);

        let kbn_notif = notifs.iter().find(|n| n.title.contains("kbn")).unwrap();
        assert_eq!(kbn_notif.count, 1);
    }

    /// Flushing with no pending primes must be a no-op — no notifications
    /// created. This prevents empty "0 primes found" notifications.
    #[test]
    fn flush_empty_is_noop() {
        let bus = make_bus();
        bus.flush();
        assert!(bus.recent_notifications(100).is_empty());
    }

    /// Flush must drain pending_primes so a second flush does not produce
    /// duplicate notifications. Uses std::mem::take internally.
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

    /// Notification detail lines are capped at 5 per form. When 8 factorial
    /// primes are flushed, the notification has 6 detail lines: 5 expressions
    /// plus "and 3 more". This prevents oversized WebSocket messages.
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

    // ── Bounded Buffer Caps ────────────────────────────────────────

    /// The event buffer is capped at RECENT_EVENTS_CAP=200 entries. Older
    /// events are evicted (pop_front) to prevent unbounded memory growth
    /// on long-running coordinators.
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

    /// The notification buffer is capped at NOTIFICATIONS_CAP=50 entries.
    /// This bounds the payload size for new WebSocket connections that
    /// receive the backlog of recent notifications.
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

    // ── Ordering ──────────────────────────────────────────────────

    /// recent_events returns events in reverse chronological order (most
    /// recent first). The dashboard event timeline displays events in this
    /// order, with the newest at the top.
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

    /// Notification IDs must be monotonically increasing (assigned via
    /// AtomicU64::fetch_add). The frontend uses these IDs for deduplication
    /// when reconnecting to the WebSocket.
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

    // ── WebSocket Broadcast ────────────────────────────────────────

    /// Verifies the full WebSocket delivery path: set_ws_sender, subscribe,
    /// emit a milestone event, and verify the subscriber receives a JSON
    /// message containing the event data. This is the primary real-time
    /// notification channel for the Next.js frontend.
    #[test]
    fn ws_subscribe_receives_notifications() {
        let bus = make_bus();
        let (tx, _rx) = tokio::sync::broadcast::channel::<String>(16);
        bus.set_ws_sender(tx);
        let mut receiver = bus.subscribe_ws();

        bus.emit(Event::Milestone {
            message: "ws-test".into(),
            timestamp: Instant::now(),
        });

        let msg = receiver.try_recv();
        assert!(msg.is_ok(), "Should receive a broadcast message");
        let json_str = msg.unwrap();
        assert!(json_str.contains("ws-test"));
        assert!(json_str.contains("milestone"));
    }

    /// Multiple WebSocket subscribers must all receive every notification.
    /// The tokio::sync::broadcast channel provides multi-consumer delivery.
    /// Each dashboard tab creates its own subscriber.
    #[test]
    fn ws_multiple_subscribers_all_receive() {
        let bus = make_bus();
        let (tx, _rx) = tokio::sync::broadcast::channel::<String>(16);
        bus.set_ws_sender(tx);

        let mut rx1 = bus.subscribe_ws();
        let mut rx2 = bus.subscribe_ws();
        let mut rx3 = bus.subscribe_ws();

        bus.emit(Event::Milestone {
            message: "multi-sub".into(),
            timestamp: Instant::now(),
        });

        assert!(rx1.try_recv().is_ok());
        assert!(rx2.try_recv().is_ok());
        assert!(rx3.try_recv().is_ok());
    }

    /// Without set_ws_sender (e.g., running as a standalone worker without
    /// a dashboard), emitting events must not panic. Events are still
    /// recorded in the buffer; only WebSocket delivery is skipped.
    #[test]
    fn ws_no_sender_does_not_panic() {
        // Without set_ws_sender, emitting events should not panic
        let bus = make_bus();
        bus.emit(Event::Milestone {
            message: "no-ws".into(),
            timestamp: Instant::now(),
        });
        // Just verifying no panic occurs
        let events = bus.recent_events(10);
        assert_eq!(events.len(), 1);
    }

    /// PrimeFound events send two WebSocket messages:
    /// 1. An immediate `prime_found` event (for real-time subscriptions,
    ///    replacing Supabase Realtime INSERT).
    /// 2. A batched `notification` on flush() (squashed by form for the
    ///    notification toast system).
    #[test]
    fn ws_prime_broadcasts_immediate_and_batched() {
        let bus = make_bus();
        let (tx, _rx) = tokio::sync::broadcast::channel::<String>(16);
        bus.set_ws_sender(tx);
        let mut receiver = bus.subscribe_ws();

        bus.emit(prime_event("factorial", "5!+1"));

        // Immediate prime_found event is broadcast right away
        let msg = receiver.try_recv();
        assert!(msg.is_ok(), "prime_found should be broadcast immediately");
        let json_str = msg.unwrap();
        assert!(json_str.contains("\"type\":\"prime_found\""));
        assert!(json_str.contains("factorial"));

        // No batched notification yet (that requires flush)
        let msg2 = receiver.try_recv();
        assert!(msg2.is_err(), "Batched notification should not appear before flush");

        bus.flush();

        // After flush, the batched notification is broadcast
        let msg3 = receiver.try_recv();
        assert!(msg3.is_ok(), "Batched notification should be broadcast after flush");
        let json_str3 = msg3.unwrap();
        assert!(json_str3.contains("\"type\":\"notification\""));
        assert!(json_str3.contains("factorial"));
    }

    // ── Event Kind Filtering ──────────────────────────────────────

    /// Verifies that all 4 event types (search_start, warning, error, prime)
    /// are correctly tagged with their kind string in the event record. The
    /// dashboard uses kind-based filtering to show subsets of events.
    #[test]
    fn recent_events_contain_correct_kinds() {
        let bus = make_bus();

        bus.emit(Event::SearchStarted {
            search_type: "kbn".into(),
            params: "k=1 b=2".into(),
            timestamp: Instant::now(),
        });
        bus.emit(Event::Warning {
            context: "sieve".into(),
            message: "warning msg".into(),
            timestamp: Instant::now(),
        });
        bus.emit(Event::Error {
            context: "db".into(),
            message: "error msg".into(),
            timestamp: Instant::now(),
        });
        bus.emit(prime_event("factorial", "7!+1"));

        let events = bus.recent_events(100);
        assert_eq!(events.len(), 4);

        let kinds: Vec<&str> = events.iter().map(|e| e.kind.as_str()).collect();
        assert!(kinds.contains(&"search_start"));
        assert!(kinds.contains(&"warning"));
        assert!(kinds.contains(&"error"));
        assert!(kinds.contains(&"prime"));
    }

    /// The limit parameter on recent_events must be respected — requesting
    /// 3 from a buffer of 10 returns exactly 3 (the most recent).
    #[test]
    fn recent_events_limit_respected() {
        let bus = make_bus();
        for i in 0..10 {
            bus.emit(Event::Warning {
                context: "test".into(),
                message: format!("msg {}", i),
                timestamp: Instant::now(),
            });
        }
        let events = bus.recent_events(3);
        assert_eq!(events.len(), 3);
        // Should return the 3 most recent (reversed order)
        assert!(events[0].message.contains("msg 9"));
    }

    // ── Polling API (recent_events_since) ──────────────────────────

    /// recent_events_since returns only events with id > last_id, in
    /// ascending order. This is the polling API: the frontend sends its
    /// last-seen event ID and receives only new events since then.
    #[test]
    fn recent_events_since_returns_newer_events() {
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
        bus.emit(Event::Warning {
            context: "c".into(),
            message: "third".into(),
            timestamp: Instant::now(),
        });

        let all = bus.recent_events(100);
        // all is most-recent-first: [third, second, first]
        let first_id = all.last().unwrap().id;

        let since = bus.recent_events_since(first_id, 100);
        assert_eq!(since.len(), 2); // second, third
        // Results should be in ascending order
        assert!(since[0].id < since[1].id);
        assert!(since[0].message.contains("second"));
        assert!(since[1].message.contains("third"));
    }

    /// Passing last_id=0 returns all events (since all IDs start at 1).
    /// This is used for the initial load when a frontend connects for the
    /// first time and has no last-seen ID.
    #[test]
    fn recent_events_since_zero_returns_all() {
        let bus = make_bus();
        bus.emit(Event::Milestone {
            message: "one".into(),
            timestamp: Instant::now(),
        });
        bus.emit(Event::Milestone {
            message: "two".into(),
            timestamp: Instant::now(),
        });

        let since = bus.recent_events_since(0, 100);
        assert_eq!(since.len(), 2);
    }

    /// When more events exist than the limit, recent_events_since returns
    /// only the N most recent (in ascending order). This bounds the response
    /// payload for long-polling clients that fall behind.
    #[test]
    fn recent_events_since_limit_applied() {
        let bus = make_bus();
        for i in 0..10 {
            bus.emit(Event::Warning {
                context: "test".into(),
                message: format!("msg {}", i),
                timestamp: Instant::now(),
            });
        }

        let since = bus.recent_events_since(0, 3);
        assert_eq!(since.len(), 3);
        // Should return the 3 most recent in ascending order
        assert!(since[0].id < since[1].id);
        assert!(since[1].id < since[2].id);
    }

    /// A last_id far beyond the current maximum returns an empty set.
    /// This handles the edge case where a client sends a stale ID from
    /// a previous coordinator session.
    #[test]
    fn recent_events_since_future_id_returns_empty() {
        let bus = make_bus();
        bus.emit(Event::Warning {
            context: "a".into(),
            message: "test".into(),
            timestamp: Instant::now(),
        });
        let since = bus.recent_events_since(999999, 100);
        assert!(since.is_empty());
    }

    // ── Event Record IDs ──────────────────────────────────────────

    /// Event IDs must be monotonically increasing (assigned via separate
    /// AtomicU64 next_event_id counter). Since recent_events returns
    /// most-recent-first, the IDs appear in decreasing order.
    #[test]
    fn event_ids_are_monotonically_increasing() {
        let bus = make_bus();
        for _ in 0..5 {
            bus.emit(Event::Warning {
                context: "test".into(),
                message: "msg".into(),
                timestamp: Instant::now(),
            });
        }
        let events = bus.recent_events(10);
        // events is most-recent-first, so ids should be decreasing
        for i in 0..events.len() - 1 {
            assert!(events[i].id > events[i + 1].id);
        }
    }

    // ── Concurrent Safety ─────────────────────────────────────────

    /// 8 threads each emit 50 warning events concurrently. All emits must
    /// complete without panics, and the event buffer must be exactly at its
    /// cap of 200 (400 total emitted, oldest 200 evicted). This validates
    /// that Mutex-guarded internal state handles high contention correctly.
    #[test]
    fn concurrent_emits_do_not_panic() {
        use std::sync::Arc;
        use std::thread;

        let bus = Arc::new(make_bus());
        let handles: Vec<_> = (0..8)
            .map(|i| {
                let bus = Arc::clone(&bus);
                thread::spawn(move || {
                    for j in 0..50 {
                        bus.emit(Event::Warning {
                            context: format!("thread-{}", i),
                            message: format!("msg-{}", j),
                            timestamp: Instant::now(),
                        });
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        let events = bus.recent_events(300);
        // Capped at 200, but should have at least that many
        assert_eq!(events.len(), RECENT_EVENTS_CAP);
    }

    // ── Default Trait ─────────────────────────────────────────────

    /// EventBus::default() must produce an empty bus identical to new().
    /// Used in struct initialization (e.g., AppState { events: EventBus::default(), ... }).
    #[test]
    fn event_bus_default_is_empty() {
        let bus = EventBus::default();
        assert!(bus.recent_events(100).is_empty());
        assert!(bus.recent_notifications(100).is_empty());
    }

    // ── Utility Functions ──────────────────────────────────────────

    /// elapsed_tag formats an Instant as "HH:MM:SS" for log output. Called
    /// immediately after creation, the tag should be "00:00:0X" (sub-second
    /// granularity means the last digit may vary).
    #[test]
    fn elapsed_tag_formats_correctly() {
        // Test the elapsed_tag helper with a known start time
        let start = Instant::now();
        let tag = elapsed_tag(start);
        // Should be "00:00:00" (or very close to it)
        assert_eq!(tag.len(), 8); // "HH:MM:SS"
        assert!(tag.starts_with("00:00:0"));
    }

    // ── Search Completion Details ──────────────────────────────────

    /// SearchCompleted notification details must include tested count, found
    /// count, and elapsed time. Elapsed is formatted to 1 decimal ("123.5s").
    /// These stats appear in the dashboard notification toast.
    #[test]
    fn search_completed_notification_contains_stats() {
        let bus = make_bus();
        bus.emit(Event::SearchCompleted {
            search_type: "palindromic".into(),
            tested: 50000,
            found: 12,
            elapsed_secs: 123.456,
            timestamp: Instant::now(),
        });

        let notifs = bus.recent_notifications(10);
        assert_eq!(notifs.len(), 1);
        assert!(notifs[0].title.contains("palindromic"));
        assert!(notifs[0].details[0].contains("50000"));
        assert!(notifs[0].details[0].contains("12"));
        assert!(notifs[0].details[0].contains("123.5")); // rounded
    }

    // ── Expression Truncation (sensitive data protection) ────────────

    /// Short expressions (under 1000 chars) should be recorded verbatim in the
    /// event log. The truncation logic only activates for very long expressions
    /// that would bloat structured log output (e.g., 100K+ digit palindromic
    /// primes stored as literal decimal strings).
    #[test]
    fn emit_prime_short_expression_not_truncated() {
        let bus = make_bus();
        bus.emit(prime_event("factorial", "100!+1"));
        let events = bus.recent_events(10);
        assert_eq!(events.len(), 1);
        assert!(events[0].message.contains("100!+1"));
    }

    /// Expressions longer than 1000 characters are truncated in the tracing
    /// log output (the `info!` macro call), but the full expression is still
    /// stored in the event record and pending_primes buffer for WebSocket
    /// delivery. This test verifies the event record retains the full expression.
    #[test]
    fn emit_prime_long_expression_still_in_event_record() {
        let bus = make_bus();
        let long_expr = "9".repeat(2000);
        bus.emit(Event::PrimeFound {
            form: "palindromic".into(),
            expression: long_expr.clone(),
            digits: 2000,
            proof_method: "probabilistic".into(),
            timestamp: Instant::now(),
        });
        let events = bus.recent_events(10);
        assert_eq!(events.len(), 1);
        // The event record message should contain the full expression
        // (truncation only applies to the tracing log, not the event buffer)
        assert!(events[0].message.contains(&long_expr));
    }
}
