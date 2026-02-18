//! # Agent — Claude Code Subprocess Manager
//!
//! Spawns and manages Claude Code CLI processes as autonomous agents that
//! work on tasks from the dashboard. Each agent runs as a `claude` subprocess
//! with injected context (CLAUDE.md files, roadmaps) matched to the task's
//! detected domain (engine, frontend, deploy, etc.).
//!
//! ## Architecture
//!
//! ```text
//! Dashboard API → AgentManager::spawn_agent() → tokio::process::Command
//!                 AgentManager::poll_agents()  → stdout streaming → DB updates
//!                 AgentManager::cancel_agent() → Child::kill()
//! ```
//!
//! ## Context Injection
//!
//! [`detect_domains()`] scans the task title and description for domain keywords
//! (e.g., "sieve", "factorial" → engine; "deploy", "ssh" → deploy) and includes
//! the matching CLAUDE.md and roadmap files via `--allowedTools` and system
//! prompt injection. Up to [`MAX_AGENTS`] run concurrently.
//!
//! ## Task Lifecycle
//!
//! Tasks are stored in PostgreSQL (`agent_tasks` table) and transition through:
//! `pending` → `running` → `completed` / `failed` / `cancelled`.
//! Agent stdout is streamed line-by-line into the `output` column for
//! real-time display on the dashboard.

use crate::db::{AgentTaskRow, Database};
use chrono::Utc;
use serde::Serialize;
use std::collections::HashMap;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::task::JoinHandle;

pub const MAX_AGENTS: usize = 2;
pub const DEFAULT_TIMEOUT_SECS: u64 = 1800;

// --- Context injection: file mappings and domain detection ---

/// CLAUDE.md files per domain. The root file is always included; domain files are included
/// when `detect_domains()` matches. "server" shares `src/CLAUDE.md` with "engine".
const CONTEXT_FILES: &[(&str, &str)] = &[
    ("root", "CLAUDE.md"),
    ("engine", "src/CLAUDE.md"),
    ("server", "src/CLAUDE.md"),
    ("frontend", "frontend/CLAUDE.md"),
    ("deploy", "deploy/CLAUDE.md"),
    ("docs", "docs/CLAUDE.md"),
];

/// Roadmap files per domain. Included when the domain is detected.
const ROADMAP_FILES: &[(&str, &str)] = &[
    ("engine", "docs/roadmaps/engine.md"),
    ("frontend", "docs/roadmaps/frontend.md"),
    ("server", "docs/roadmaps/server.md"),
    ("deploy", "docs/roadmaps/ops.md"),
    ("docs", "docs/roadmaps/research.md"),
];

/// Maximum number of lines to include from a roadmap file.
const ROADMAP_MAX_LINES: usize = 100;

/// Scan task title and description for domain keywords.
/// Returns a list of matched domain names (engine, frontend, deploy, docs, server).
pub fn detect_domains(title: &str, description: &str) -> Vec<&'static str> {
    let text = format!("{} {}", title, description).to_lowercase();
    let mut domains = Vec::new();

    let engine_keywords = [
        "sieve", "primality", "factorial", "kbn", "gmp", "rug", "proof", "algorithm",
        "palindromic", "proth", "llr", "pocklington", "montgomery", "morrison",
        "repunit", "wagstaff", "cullen", "woodall", "carol", "kynea", "twin",
        "sophie", "germain", "fermat", "primorial", "near_repdigit", "near-repdigit",
    ];
    if engine_keywords.iter().any(|kw| text.contains(kw)) {
        domains.push("engine");
    }

    let frontend_keywords = [
        "react", "next.js", "nextjs", "component", "dashboard", "chart", "ui",
        "tailwind", "frontend", "recharts", "shadcn", "page.tsx", "hook",
    ];
    if frontend_keywords.iter().any(|kw| text.contains(kw)) {
        domains.push("frontend");
    }

    let deploy_keywords = [
        "deploy", "systemd", "ssh", "production", "ops", "build", "release",
        "pgo", "mimalloc", "cargo build", "binary", "service",
    ];
    if deploy_keywords.iter().any(|kw| text.contains(kw)) {
        domains.push("deploy");
    }

    let docs_keywords = [
        "research", "oeis", "paper", "publication", "strategy", "record",
        "documentation", "roadmap",
    ];
    if docs_keywords.iter().any(|kw| text.contains(kw)) {
        domains.push("docs");
    }

    let server_keywords = [
        "api", "websocket", "axum", "database", "postgres", "coordination",
        "fleet", "worker", "endpoint", "rest", "sqlx", "migration",
    ];
    if server_keywords.iter().any(|kw| text.contains(kw)) {
        domains.push("server");
    }

    domains
}

/// Assemble context prompt sections for a spawning agent.
///
/// Returns a `Vec<String>` where each element becomes a separate `--append-system-prompt`
/// argument. Sections include: project CLAUDE.md files, relevant roadmaps, recent git
/// history, accumulated agent memory, and task history (siblings + previous failures).
pub async fn assemble_context(task: &AgentTaskRow, db: &Database) -> Vec<String> {
    let mut sections = Vec::new();
    let domains = detect_domains(&task.title, &task.description);

    // 1. Project context files (root CLAUDE.md always; domain files if matched)
    let mut included_paths = Vec::new();
    for &(domain, path) in CONTEXT_FILES {
        if domain == "root" || domains.contains(&domain) {
            // Avoid duplicating src/CLAUDE.md if both engine and server are detected
            if included_paths.contains(&path) {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(path) {
                sections.push(format!(
                    "# Project Context: {}\n\n{}",
                    path, content
                ));
                included_paths.push(path);
                eprintln!("  Context: included {}", path);
            }
        }
    }

    // 2. Relevant roadmaps (truncated to first N lines)
    for &(domain, path) in ROADMAP_FILES {
        if domains.contains(&domain) {
            if let Ok(content) = std::fs::read_to_string(path) {
                let truncated: String = content
                    .lines()
                    .take(ROADMAP_MAX_LINES)
                    .collect::<Vec<_>>()
                    .join("\n");
                let suffix = if content.lines().count() > ROADMAP_MAX_LINES {
                    format!("\n\n... (truncated at {} lines)", ROADMAP_MAX_LINES)
                } else {
                    String::new()
                };
                sections.push(format!(
                    "# Roadmap: {}\n\n{}{}",
                    path, truncated, suffix
                ));
                eprintln!("  Context: included roadmap {}", path);
            }
        }
    }

    // 3. Recent git history (best-effort, skip on failure)
    if let Ok(output) = std::process::Command::new("git")
        .args(["log", "--oneline", "-10"])
        .output()
    {
        if output.status.success() {
            let log = String::from_utf8_lossy(&output.stdout);
            if !log.trim().is_empty() {
                sections.push(format!(
                    "# Recent Git History\n\n```\n{}\n```",
                    log.trim()
                ));
            }
        }
    }

    // 4. Agent memory (all entries, grouped by category)
    if let Ok(memories) = db.get_all_agent_memory().await {
        if !memories.is_empty() {
            let mut memory_text = String::from("# Agent Memory\n\nAccumulated knowledge from previous agent tasks:\n\n");
            let mut current_category = String::new();
            for mem in &memories {
                if mem.category != current_category {
                    current_category.clone_from(&mem.category);
                    memory_text.push_str(&format!("## {}\n\n", current_category));
                }
                memory_text.push_str(&format!("- **{}**: {}\n", mem.key, mem.value));
            }
            memory_text.push_str(&format!(
                "\nTo save new knowledge, use SQL:\n```sql\nINSERT INTO agent_memory (key, value, category, created_by_task)\nVALUES ('key', 'value', 'category', {})\nON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, category = EXCLUDED.category, updated_at = now();\n```",
                task.id
            ));
            sections.push(memory_text);
            eprintln!("  Context: included {} memory entries", memories.len());
        }
    }

    // 5. Task history: parent task, siblings, completed step results, previous failed attempts
    let mut history_parts = Vec::new();

    // Parent task context
    if let Some(parent_id) = task.parent_task_id {
        if let Ok(Some(parent)) = db.get_agent_task(parent_id).await {
            history_parts.push(format!(
                "## Parent Task #{}\n\nTitle: {}\nDescription: {}\nStatus: {}",
                parent.id, parent.title, parent.description, parent.status
            ));
        }

        // Sibling tasks (other tasks with same parent)
        if let Ok(siblings) = db.get_sibling_tasks(parent_id, task.id).await {
            if !siblings.is_empty() {
                let mut sibling_text = String::from("## Sibling Tasks\n\n");
                for s in &siblings {
                    sibling_text.push_str(&format!(
                        "- #{} [{}] {}\n",
                        s.id, s.status, s.title
                    ));
                }
                history_parts.push(sibling_text);
            }

            // Inject completed sibling results for template workflows.
            // This lets later steps build on earlier step outputs.
            let completed_siblings: Vec<_> = siblings
                .iter()
                .filter(|s| s.status == "completed" && s.result.is_some())
                .collect();
            if !completed_siblings.is_empty() {
                let mut results_text = String::from(
                    "## Completed Steps\n\nResults from earlier steps in this workflow:\n\n",
                );
                for s in &completed_siblings {
                    let result_summary = s
                        .result
                        .as_ref()
                        .and_then(|r| r.get("text"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("(no text output)");
                    // Truncate very long results to keep context manageable
                    let truncated = if result_summary.len() > 2000 {
                        format!("{}...\n(truncated)", &result_summary[..2000])
                    } else {
                        result_summary.to_string()
                    };
                    results_text.push_str(&format!(
                        "### Step: {}\n\n{}\n\n",
                        s.title, truncated
                    ));
                }
                history_parts.push(results_text);
                eprintln!(
                    "  Context: included {} completed step results",
                    completed_siblings.len()
                );
            }
        }
    }

    // Previous failed attempts with same title
    if let Ok(prev) = db.get_previous_attempts(&task.title, task.id).await {
        if !prev.is_empty() {
            let mut prev_text = String::from("## PREVIOUS FAILED ATTEMPTS\n\nThis task has been attempted before and failed. Learn from these failures:\n\n");
            for p in &prev {
                let result_summary = p
                    .result
                    .as_ref()
                    .and_then(|r| r.get("error"))
                    .and_then(|e| e.as_str())
                    .or_else(|| p.result.as_ref().and_then(|r| r.get("text")).and_then(|t| t.as_str()))
                    .unwrap_or("(no details)");
                prev_text.push_str(&format!(
                    "### Attempt #{} ({})\n- Status: {}\n- Result: {}\n\n",
                    p.id,
                    p.created_at.format("%Y-%m-%d %H:%M"),
                    p.status,
                    result_summary,
                ));
            }
            history_parts.push(prev_text);
            eprintln!("  Context: included {} previous failed attempts", prev.len());
        }
    }

    if !history_parts.is_empty() {
        sections.push(format!("# Task History\n\n{}", history_parts.join("\n")));
    }

    eprintln!("  Context: assembled {} sections for task {}", sections.len(), task.id);
    sections
}

/// Result accumulated from reading the claude CLI's NDJSON stdout stream.
#[derive(Debug, Default)]
pub struct StdoutResult {
    pub result_text: String,
    pub tokens_used: i64,
    pub cost_usd: f64,
}

/// Serializable summary of a running agent for the WebSocket/API.
#[derive(Clone, Serialize)]
pub struct AgentInfo {
    pub task_id: i64,
    pub title: String,
    pub model: String,
    pub status: AgentStatus,
    pub started_at: String,
    pub pid: Option<u32>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Running,
    Completed,
    Failed { reason: String },
    Cancelled,
    TimedOut,
}

struct AgentEntry {
    info: AgentInfo,
    child: Child,
    stdout_handle: JoinHandle<StdoutResult>,
    timeout_at: chrono::DateTime<Utc>,
    started_at_utc: chrono::DateTime<Utc>,
    max_cost_usd: Option<f64>,
    cost_rate_per_sec: f64,
}

/// Conservative cost rate estimates per second by model.
/// These are intentionally high so we over-kill rather than overspend.
pub fn estimated_cost_per_sec(model: &str) -> f64 {
    match model {
        "opus" => 0.015,   // ~$0.90/min
        "sonnet" => 0.005, // ~$0.30/min
        "haiku" => 0.001,  // ~$0.06/min
        _ => 0.005,        // default to sonnet rate
    }
}

/// Completed agent info returned by poll_completed().
pub struct CompletedAgent {
    pub task_id: i64,
    pub status: AgentStatus,
    pub result: Option<StdoutResult>,
}

/// Returns the --tools whitelist for a permission level.
fn tools_for_level(level: i32) -> &'static str {
    match level {
        0 => "Read,Grep,Glob,WebSearch,WebFetch",
        1 => "Read,Write,Edit,Grep,Glob,Bash,WebSearch,WebFetch",
        2 => "Read,Write,Edit,Grep,Glob,Bash,WebSearch,WebFetch,Task",
        _ => "default", // Level 3 = all tools
    }
}

/// Returns a safety system prompt for the permission level.
fn safety_prompt_for_level(level: i32) -> &'static str {
    match level {
        0 => "You are in READ-ONLY mode. Do NOT create, modify, or delete any files.",
        1 => "SAFETY: Do NOT run destructive commands (rm -rf, sudo). Do NOT use git push, git checkout ., or git reset --hard. Only work on branches, never main/master.",
        2 => "SAFETY: Work on branch agent/<task-id>. Never force-push. Never push to main/master. Tag commits with Co-Authored-By.",
        _ => "", // Level 3 = no restrictions
    }
}

#[derive(Default)]
pub struct AgentManager {
    agents: HashMap<i64, AgentEntry>,
}

impl AgentManager {
    pub fn new() -> Self {
        AgentManager {
            agents: HashMap::new(),
        }
    }

    pub fn active_count(&self) -> usize {
        self.agents.len()
    }

    /// Spawn a claude CLI subprocess for the given task. The db is passed so the
    /// async stdout reader can insert events directly. `context_prompts` are additional
    /// system prompt sections assembled by `assemble_context()`.
    pub fn spawn_agent(
        &mut self,
        task: &AgentTaskRow,
        db: Database,
        max_cost_usd: Option<f64>,
        context_prompts: Vec<String>,
    ) -> Result<AgentInfo, String> {
        if self.agents.contains_key(&task.id) {
            return Err(format!("Agent already running for task {}", task.id));
        }

        let model = task.agent_model.as_deref().unwrap_or("sonnet");
        let prompt = if task.description.is_empty() {
            task.title.clone()
        } else {
            format!("{}\n\n{}", task.title, task.description)
        };

        let mut cmd = Command::new("claude");
        cmd.arg("-p")
            .arg(&prompt)
            .arg("--model")
            .arg(model)
            .arg("--output-format")
            .arg("stream-json")
            .arg("--max-turns")
            .arg("50");

        // Tool restriction based on permission level
        let level = task.permission_level;
        cmd.arg("--tools").arg(tools_for_level(level));

        // Safety system prompt
        let safety = safety_prompt_for_level(level);
        if !safety.is_empty() {
            cmd.arg("--append-system-prompt").arg(safety);
        }

        // Inject context sections (project files, roadmaps, memory, task history)
        for section in &context_prompts {
            cmd.arg("--append-system-prompt").arg(section);
        }

        // Levels 1+ get autonomous execution (no interactive permission prompts)
        if level >= 1 {
            cmd.arg("--dangerously-skip-permissions");
        }

        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::null());
        cmd.kill_on_drop(true);

        let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn claude: {}", e))?;
        let pid = child.id();

        // Take stdout immediately and hand it to an async reader task
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Failed to capture stdout".to_string())?;

        let task_id = task.id;
        let stdout_handle = tokio::spawn(read_agent_stdout(task_id, stdout, db));

        let now = Utc::now();
        let timeout_at = now + chrono::Duration::seconds(DEFAULT_TIMEOUT_SECS as i64);
        let cost_rate = estimated_cost_per_sec(model);

        let info = AgentInfo {
            task_id: task.id,
            title: task.title.clone(),
            model: model.to_string(),
            status: AgentStatus::Running,
            started_at: now.to_rfc3339(),
            pid,
        };

        self.agents.insert(task.id, AgentEntry {
            info: info.clone(),
            child,
            stdout_handle,
            timeout_at,
            started_at_utc: now,
            max_cost_usd,
            cost_rate_per_sec: cost_rate,
        });

        eprintln!(
            "Agent spawned for task {} (pid {:?}, model {}, level {}, timeout {}s)",
            task.id,
            pid,
            model,
            level,
            DEFAULT_TIMEOUT_SECS
        );

        Ok(info)
    }

    /// Non-blocking poll: check for completed/timed-out/over-budget agents.
    /// Returns entries that finished so the caller can update the DB.
    pub fn poll_completed(&mut self) -> Vec<CompletedAgent> {
        let now = Utc::now();
        let mut finished = Vec::new();
        let mut to_remove = Vec::new();

        for (task_id, entry) in &mut self.agents {
            // Check per-task budget first (estimated cost based on elapsed time)
            if let Some(max_cost) = entry.max_cost_usd {
                let elapsed_secs = (now - entry.started_at_utc).num_seconds().max(0) as f64;
                let estimated_cost = elapsed_secs * entry.cost_rate_per_sec;
                if estimated_cost > max_cost {
                    let _ = entry.child.start_kill();
                    entry.stdout_handle.abort();
                    to_remove.push(*task_id);
                    finished.push(CompletedAgent {
                        task_id: *task_id,
                        status: AgentStatus::Failed {
                            reason: format!(
                                "budget_exceeded: estimated ${:.2} > max ${:.2}",
                                estimated_cost, max_cost
                            ),
                        },
                        result: None,
                    });
                    continue;
                }
            }

            // Check timeout
            if now >= entry.timeout_at {
                let _ = entry.child.start_kill();
                entry.stdout_handle.abort();
                to_remove.push(*task_id);
                finished.push(CompletedAgent {
                    task_id: *task_id,
                    status: AgentStatus::TimedOut,
                    result: None,
                });
                continue;
            }

            // Non-blocking check if process exited
            match entry.child.try_wait() {
                Ok(Some(exit_status)) => {
                    // Process exited — try to get stdout result
                    let result = if entry.stdout_handle.is_finished() {
                        // Use now_or_never to avoid blocking
                        match futures_util_now_or_never(&mut entry.stdout_handle) {
                            Some(Ok(r)) => Some(r),
                            Some(Err(_)) => None,
                            None => None,
                        }
                    } else {
                        None
                    };

                    let status = if exit_status.success() {
                        AgentStatus::Completed
                    } else {
                        AgentStatus::Failed {
                            reason: format!("Exit code: {}", exit_status),
                        }
                    };

                    to_remove.push(*task_id);
                    finished.push(CompletedAgent {
                        task_id: *task_id,
                        status,
                        result,
                    });
                }
                Ok(None) => {} // still running
                Err(e) => {
                    to_remove.push(*task_id);
                    finished.push(CompletedAgent {
                        task_id: *task_id,
                        status: AgentStatus::Failed {
                            reason: format!("Wait error: {}", e),
                        },
                        result: None,
                    });
                }
            }
        }

        for id in to_remove {
            self.agents.remove(&id);
        }

        finished
    }

    /// Kill a running agent. Returns true if found and killed.
    pub fn cancel_agent(&mut self, task_id: i64) -> bool {
        if let Some(mut entry) = self.agents.remove(&task_id) {
            let _ = entry.child.start_kill();
            entry.stdout_handle.abort();
            eprintln!("Agent for task {} cancelled", task_id);
            true
        } else {
            false
        }
    }

    /// Kill all running agents (for global budget enforcement).
    /// Returns task IDs of killed agents.
    pub fn kill_all(&mut self) -> Vec<i64> {
        let task_ids: Vec<i64> = self.agents.keys().copied().collect();
        for entry in self.agents.values_mut() {
            let _ = entry.child.start_kill();
            entry.stdout_handle.abort();
        }
        self.agents.clear();
        task_ids
    }

    /// Get serializable info for all running agents.
    pub fn get_all(&self) -> Vec<AgentInfo> {
        self.agents.values().map(|e| e.info.clone()).collect()
    }
}

impl Drop for AgentManager {
    fn drop(&mut self) {
        for entry in self.agents.values_mut() {
            let _ = entry.child.start_kill();
            entry.stdout_handle.abort();
        }
    }
}

/// Synchronously check if a JoinHandle is ready without blocking.
/// This avoids needing the futures crate — just polls once.
fn futures_util_now_or_never<T>(handle: &mut JoinHandle<T>) -> Option<Result<T, tokio::task::JoinError>> {
    // Use a tiny runtime-free check: if the handle is finished, we can block briefly
    if handle.is_finished() {
        // The task is done, so this won't actually block
        Some(handle.try_join())
    } else {
        None
    }
}

/// Trait extension to try joining a finished handle.
trait JoinHandleExt<T> {
    fn try_join(&mut self) -> Result<T, tokio::task::JoinError>;
}

impl<T> JoinHandleExt<T> for JoinHandle<T> {
    fn try_join(&mut self) -> Result<T, tokio::task::JoinError> {
        // Since we only call this after is_finished() returns true,
        // a blocking approach is safe. We use tokio::task::block_in_place
        // to avoid panics if called from a tokio context.
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self)
        })
    }
}

/// Async function that reads NDJSON from the claude CLI's stdout,
/// inserts events into the database, and returns accumulated results.
async fn read_agent_stdout(
    task_id: i64,
    stdout: tokio::process::ChildStdout,
    db: Database,
) -> StdoutResult {
    let mut reader = BufReader::new(stdout).lines();
    let mut result = StdoutResult::default();

    while let Ok(Some(line)) = reader.next_line().await {
        let parsed: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let msg_type = parsed.get("type").and_then(|t| t.as_str()).unwrap_or("");

        match msg_type {
            "system" => {
                let subtype = parsed.get("subtype").and_then(|s| s.as_str()).unwrap_or("");
                if subtype == "init" {
                    let _ = db
                        .insert_agent_event(
                            Some(task_id),
                            "started",
                            Some("claude"),
                            "Agent started",
                            Some(&parsed),
                        )
                        .await;
                }
            }
            "assistant" => {
                // Check for text content blocks
                if let Some(content) = parsed.get("content").and_then(|c| c.as_array()) {
                    for block in content {
                        let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        match block_type {
                            "text" => {
                                let text = block.get("text").and_then(|t| t.as_str()).unwrap_or("");
                                // Truncate long messages for the event summary
                                let summary = if text.len() > 200 {
                                    format!("{}...", &text[..200])
                                } else {
                                    text.to_string()
                                };
                                let _ = db
                                    .insert_agent_event(
                                        Some(task_id),
                                        "message",
                                        Some("claude"),
                                        &summary,
                                        Some(&parsed),
                                    )
                                    .await;
                            }
                            "tool_use" => {
                                let tool_name = block
                                    .get("name")
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("unknown");
                                let _ = db
                                    .insert_agent_event(
                                        Some(task_id),
                                        "tool_call",
                                        Some("claude"),
                                        &format!("Tool call: {}", tool_name),
                                        Some(block),
                                    )
                                    .await;
                            }
                            _ => {}
                        }
                    }
                }
            }
            "result" => {
                // Extract usage/cost from the result message
                if let Some(usage) = parsed.get("usage") {
                    let input = usage.get("input_tokens").and_then(|t| t.as_i64()).unwrap_or(0);
                    let output = usage.get("output_tokens").and_then(|t| t.as_i64()).unwrap_or(0);
                    result.tokens_used = input + output;
                }
                result.cost_usd = parsed
                    .get("cost_usd")
                    .and_then(|c| c.as_f64())
                    .unwrap_or(0.0);
                result.result_text = parsed
                    .get("result")
                    .and_then(|r| r.as_str())
                    .unwrap_or("")
                    .to_string();

                let summary = if result.result_text.len() > 200 {
                    format!("{}...", &result.result_text[..200])
                } else {
                    result.result_text.clone()
                };
                let _ = db
                    .insert_agent_event(
                        Some(task_id),
                        "completed",
                        Some("claude"),
                        &summary,
                        Some(&parsed),
                    )
                    .await;
            }
            _ => {}
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_manager_starts_empty() {
        let mgr = AgentManager::new();
        assert_eq!(mgr.active_count(), 0);
        assert!(mgr.get_all().is_empty());
    }

    #[test]
    fn agent_status_serializes_correctly() {
        let running = serde_json::to_string(&AgentStatus::Running).unwrap();
        assert!(running.contains("running"));

        let failed = serde_json::to_string(&AgentStatus::Failed {
            reason: "exit 1".into(),
        })
        .unwrap();
        assert!(failed.contains("failed"));
        assert!(failed.contains("exit 1"));

        let timed_out = serde_json::to_string(&AgentStatus::TimedOut).unwrap();
        assert!(timed_out.contains("timed_out"));

        let cancelled = serde_json::to_string(&AgentStatus::Cancelled).unwrap();
        assert!(cancelled.contains("cancelled"));
    }

    #[test]
    fn cancel_nonexistent_returns_false() {
        let mut mgr = AgentManager::new();
        assert!(!mgr.cancel_agent(999));
    }

    #[test]
    fn poll_completed_empty_returns_empty() {
        let mut mgr = AgentManager::new();
        assert!(mgr.poll_completed().is_empty());
    }

    #[test]
    fn tools_for_level_returns_correct_whitelist() {
        assert!(tools_for_level(0).contains("Read"));
        assert!(!tools_for_level(0).contains("Bash"));
        assert!(!tools_for_level(0).contains("Write"));

        assert!(tools_for_level(1).contains("Bash"));
        assert!(tools_for_level(1).contains("Write"));
        assert!(!tools_for_level(1).contains("Task"));

        assert!(tools_for_level(2).contains("Task"));
        assert!(tools_for_level(2).contains("Bash"));

        assert_eq!(tools_for_level(3), "default");
    }

    #[test]
    fn safety_prompt_for_level_varies() {
        assert!(safety_prompt_for_level(0).contains("READ-ONLY"));
        assert!(safety_prompt_for_level(1).contains("destructive"));
        assert!(safety_prompt_for_level(2).contains("branch"));
        assert!(safety_prompt_for_level(3).is_empty());
    }

    #[test]
    fn detect_domains_engine_keywords() {
        let domains = detect_domains("Optimize the sieve module", "Improve primality testing speed");
        assert!(domains.contains(&"engine"));
    }

    #[test]
    fn detect_domains_frontend_keywords() {
        let domains = detect_domains("Fix React component", "Update the dashboard chart");
        assert!(domains.contains(&"frontend"));
    }

    #[test]
    fn detect_domains_multiple() {
        let domains = detect_domains(
            "Add API endpoint for sieve stats",
            "Create a REST endpoint that returns sieve performance data for the dashboard",
        );
        assert!(domains.contains(&"engine"), "should detect engine from 'sieve'");
        assert!(domains.contains(&"server"), "should detect server from 'endpoint' or 'rest'");
    }

    #[test]
    fn detect_domains_empty() {
        let domains = detect_domains("", "");
        assert!(domains.is_empty());
    }

    #[test]
    fn detect_domains_server_keywords() {
        let domains = detect_domains("Fix database migration", "Update postgres schema");
        assert!(domains.contains(&"server"));
    }

    #[test]
    fn detect_domains_deploy_keywords() {
        let domains = detect_domains("Set up PGO build", "Configure systemd service");
        assert!(domains.contains(&"deploy"));
    }

    #[test]
    fn detect_domains_docs_keywords() {
        let domains = detect_domains("Research OEIS sequences", "Find new prime form strategies");
        assert!(domains.contains(&"docs"));
    }
}
