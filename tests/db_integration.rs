//! Database integration tests.
//!
//! All tests require TEST_DATABASE_URL to be set.
//! Run with: TEST_DATABASE_URL=postgres://... cargo test --test db_integration
//!
//! Tests should be run single-threaded to avoid conflicts:
//!   cargo test --test db_integration -- --test-threads=1

mod common;

use darkreach::db::{Database, PrimeFilter};

/// Skip the test if TEST_DATABASE_URL is not set.
macro_rules! require_db {
    () => {
        if !common::has_test_db() {
            eprintln!("Skipping: TEST_DATABASE_URL not set");
            return;
        }
    };
}

async fn setup() -> Database {
    common::setup_test_db().await
}

// --- Prime CRUD ---

#[tokio::test]
async fn connect_to_test_db() {
    require_db!();
    let _db = setup().await;
    // If we get here without panic, connection succeeded
}

#[tokio::test]
async fn insert_prime_and_retrieve() {
    require_db!();
    let db = setup().await;

    db.insert_prime(
        "factorial",
        "5! + 1",
        3,
        r#"{"form":"factorial"}"#,
        "deterministic",
        None,
    )
    .await
    .unwrap();

    let primes = db
        .get_primes_filtered(10, 0, &PrimeFilter::default())
        .await
        .unwrap();
    assert_eq!(primes.len(), 1);
    assert_eq!(primes[0].form, "factorial");
    assert_eq!(primes[0].expression, "5! + 1");
    assert_eq!(primes[0].digits, 3);
    assert_eq!(primes[0].proof_method, "deterministic");
}

#[tokio::test]
async fn insert_duplicate_expression_ignored() {
    require_db!();
    let db = setup().await;

    db.insert_prime("factorial", "5! + 1", 3, "{}", "deterministic", None)
        .await
        .unwrap();

    // insert_prime_ignore should not error on duplicate
    db.insert_prime_ignore("factorial", "5! + 1", 3, "{}", "probabilistic")
        .await
        .unwrap();

    let primes = db
        .get_primes_filtered(10, 0, &PrimeFilter::default())
        .await
        .unwrap();
    assert_eq!(primes.len(), 1);
    // Original proof_method should be preserved
    assert_eq!(primes[0].proof_method, "deterministic");
}

#[tokio::test]
async fn filter_primes_by_form() {
    require_db!();
    let db = setup().await;

    db.insert_prime("factorial", "5! + 1", 3, "{}", "det", None)
        .await
        .unwrap();
    db.insert_prime("kbn", "3*2^5+1", 2, "{}", "det", None)
        .await
        .unwrap();
    db.insert_prime("palindromic", "10301", 5, "{}", "det", None)
        .await
        .unwrap();

    let filter = PrimeFilter {
        form: Some("factorial".into()),
        ..Default::default()
    };
    let primes = db.get_primes_filtered(10, 0, &filter).await.unwrap();
    assert_eq!(primes.len(), 1);
    assert_eq!(primes[0].form, "factorial");
}

#[tokio::test]
async fn filter_primes_by_digit_range() {
    require_db!();
    let db = setup().await;

    db.insert_prime("factorial", "5! + 1", 3, "{}", "det", None)
        .await
        .unwrap();
    db.insert_prime("kbn", "3*2^100+1", 31, "{}", "det", None)
        .await
        .unwrap();
    db.insert_prime("palindromic", "10301", 5, "{}", "det", None)
        .await
        .unwrap();

    let filter = PrimeFilter {
        min_digits: Some(4),
        max_digits: Some(10),
        ..Default::default()
    };
    let primes = db.get_primes_filtered(10, 0, &filter).await.unwrap();
    assert_eq!(primes.len(), 1);
    assert_eq!(primes[0].expression, "10301");
}

#[tokio::test]
async fn filter_primes_search_text() {
    require_db!();
    let db = setup().await;

    db.insert_prime("factorial", "73! + 1", 106, "{}", "det", None)
        .await
        .unwrap();
    db.insert_prime("factorial", "73! - 1", 106, "{}", "det", None)
        .await
        .unwrap();
    db.insert_prime("kbn", "3*2^5+1", 2, "{}", "det", None)
        .await
        .unwrap();

    let filter = PrimeFilter {
        search: Some("73!".into()),
        ..Default::default()
    };
    let primes = db.get_primes_filtered(10, 0, &filter).await.unwrap();
    assert_eq!(primes.len(), 2);
}

#[tokio::test]
async fn sort_primes_ascending_descending() {
    require_db!();
    let db = setup().await;

    db.insert_prime("factorial", "A", 10, "{}", "det", None)
        .await
        .unwrap();
    db.insert_prime("factorial", "B", 20, "{}", "det", None)
        .await
        .unwrap();
    db.insert_prime("factorial", "C", 5, "{}", "det", None)
        .await
        .unwrap();

    // Ascending by digits
    let filter = PrimeFilter {
        sort_by: Some("digits".into()),
        sort_dir: Some("asc".into()),
        ..Default::default()
    };
    let primes = db.get_primes_filtered(10, 0, &filter).await.unwrap();
    assert_eq!(primes.len(), 3);
    assert_eq!(primes[0].digits, 5);
    assert_eq!(primes[1].digits, 10);
    assert_eq!(primes[2].digits, 20);

    // Descending by digits (default)
    let filter = PrimeFilter {
        sort_by: Some("digits".into()),
        ..Default::default()
    };
    let primes = db.get_primes_filtered(10, 0, &filter).await.unwrap();
    assert_eq!(primes[0].digits, 20);
    assert_eq!(primes[2].digits, 5);
}

#[tokio::test]
async fn paginate_primes() {
    require_db!();
    let db = setup().await;

    for i in 1..=5 {
        db.insert_prime("factorial", &format!("{}! + 1", i), i, "{}", "det", None)
            .await
            .unwrap();
    }

    // Page 1: limit 2, offset 0
    let page1 = db
        .get_primes_filtered(2, 0, &PrimeFilter::default())
        .await
        .unwrap();
    assert_eq!(page1.len(), 2);

    // Page 2: limit 2, offset 2
    let page2 = db
        .get_primes_filtered(2, 2, &PrimeFilter::default())
        .await
        .unwrap();
    assert_eq!(page2.len(), 2);

    // Page 3: limit 2, offset 4
    let page3 = db
        .get_primes_filtered(2, 4, &PrimeFilter::default())
        .await
        .unwrap();
    assert_eq!(page3.len(), 1);

    // No overlap between pages
    let ids1: Vec<i64> = page1.iter().map(|p| p.id).collect();
    let ids2: Vec<i64> = page2.iter().map(|p| p.id).collect();
    let ids3: Vec<i64> = page3.iter().map(|p| p.id).collect();
    for id in &ids1 {
        assert!(!ids2.contains(id));
        assert!(!ids3.contains(id));
    }
}

#[tokio::test]
async fn get_filtered_count() {
    require_db!();
    let db = setup().await;

    db.insert_prime("factorial", "5! + 1", 3, "{}", "det", None)
        .await
        .unwrap();
    db.insert_prime("kbn", "3*2^5+1", 2, "{}", "det", None)
        .await
        .unwrap();
    db.insert_prime("kbn", "3*2^7+1", 3, "{}", "det", None)
        .await
        .unwrap();

    let count = db
        .get_filtered_count(&PrimeFilter::default())
        .await
        .unwrap();
    assert_eq!(count, 3);

    let filter = PrimeFilter {
        form: Some("kbn".into()),
        ..Default::default()
    };
    let count = db.get_filtered_count(&filter).await.unwrap();
    assert_eq!(count, 2);
}

// --- Worker coordination ---

#[tokio::test]
async fn worker_upsert_and_retrieve() {
    require_db!();
    let db = setup().await;

    db.upsert_worker(
        "test-worker-1",
        "host1.example.com",
        8,
        "factorial",
        r#"{"start":1}"#,
    )
    .await
    .unwrap();

    let workers = db.get_all_workers().await.unwrap();
    assert_eq!(workers.len(), 1);
    assert_eq!(workers[0].worker_id, "test-worker-1");
    assert_eq!(workers[0].hostname, "host1.example.com");
    assert_eq!(workers[0].cores, 8);
    assert_eq!(workers[0].search_type, "factorial");

    // Upsert same worker updates fields
    db.upsert_worker(
        "test-worker-1",
        "host1-updated.com",
        16,
        "kbn",
        r#"{"k":3}"#,
    )
    .await
    .unwrap();

    let workers = db.get_all_workers().await.unwrap();
    assert_eq!(workers.len(), 1);
    assert_eq!(workers[0].hostname, "host1-updated.com");
    assert_eq!(workers[0].cores, 16);
}

#[tokio::test]
async fn worker_delete() {
    require_db!();
    let db = setup().await;

    db.upsert_worker("w1", "host1", 4, "factorial", "")
        .await
        .unwrap();
    db.upsert_worker("w2", "host2", 8, "kbn", "").await.unwrap();

    db.delete_worker("w1").await.unwrap();

    let workers = db.get_all_workers().await.unwrap();
    assert_eq!(workers.len(), 1);
    assert_eq!(workers[0].worker_id, "w2");
}

#[tokio::test]
async fn worker_set_and_clear_command() {
    require_db!();
    let db = setup().await;

    db.upsert_worker("w1", "host1", 4, "factorial", "")
        .await
        .unwrap();

    // Set a stop command
    db.set_worker_command("w1", "stop").await.unwrap();

    // Heartbeat RPC should return and clear the command
    let cmd = db
        .worker_heartbeat_rpc(
            "w1",
            "host1",
            4,
            "factorial",
            "",
            100,
            5,
            "testing",
            None,
            None,
        )
        .await
        .unwrap();
    assert_eq!(cmd, Some("stop".to_string()));

    // Second heartbeat should return None (command was cleared)
    let cmd = db
        .worker_heartbeat_rpc(
            "w1",
            "host1",
            4,
            "factorial",
            "",
            200,
            5,
            "testing",
            None,
            None,
        )
        .await
        .unwrap();
    assert_eq!(cmd, None);
}

#[tokio::test]
async fn worker_prune_stale() {
    require_db!();
    let db = setup().await;

    db.upsert_worker("fresh", "host1", 4, "factorial", "")
        .await
        .unwrap();
    db.upsert_worker("stale", "host2", 4, "factorial", "")
        .await
        .unwrap();

    // Make the 'stale' worker's heartbeat old
    sqlx::query(
        "UPDATE workers SET last_heartbeat = NOW() - INTERVAL '300 seconds' WHERE worker_id = 'stale'",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let pruned = db.prune_stale_workers(120).await.unwrap();
    assert_eq!(pruned, 1);

    let workers = db.get_all_workers().await.unwrap();
    assert_eq!(workers.len(), 1);
    assert_eq!(workers[0].worker_id, "fresh");
}

// --- Search jobs ---

#[tokio::test]
async fn search_job_create_with_blocks() {
    require_db!();
    let db = setup().await;

    let params = serde_json::json!({"form": "factorial", "start": 1, "end": 1000});
    let job_id = db
        .create_search_job("factorial", &params, 1, 1000, 100)
        .await
        .unwrap();

    let job = db.get_search_job(job_id).await.unwrap().unwrap();
    assert_eq!(job.search_type, "factorial");
    assert_eq!(job.status, "running");
    assert_eq!(job.range_start, 1);
    assert_eq!(job.range_end, 1000);
    assert_eq!(job.block_size, 100);

    let summary = db.get_job_block_summary(job_id).await.unwrap();
    assert_eq!(summary.available, 10); // 1000/100 = 10 blocks
    assert_eq!(summary.claimed, 0);
    assert_eq!(summary.completed, 0);
}

#[tokio::test]
async fn search_job_status_lifecycle() {
    require_db!();
    let db = setup().await;

    let params = serde_json::json!({"form": "factorial"});
    let job_id = db
        .create_search_job("factorial", &params, 1, 100, 10)
        .await
        .unwrap();

    // Initially running
    let job = db.get_search_job(job_id).await.unwrap().unwrap();
    assert_eq!(job.status, "running");
    assert!(job.stopped_at.is_none());

    // Cancel it
    db.update_search_job_status(job_id, "cancelled", None)
        .await
        .unwrap();

    let job = db.get_search_job(job_id).await.unwrap().unwrap();
    assert_eq!(job.status, "cancelled");
    assert!(job.stopped_at.is_some());
}

#[tokio::test]
async fn get_job_block_summary_after_completion() {
    require_db!();
    let db = setup().await;

    // Create a worker (needed for claim_work_block foreign key)
    db.upsert_worker("block-worker", "host", 4, "factorial", "")
        .await
        .unwrap();

    let params = serde_json::json!({"form": "factorial"});
    let job_id = db
        .create_search_job("factorial", &params, 1, 30, 10)
        .await
        .unwrap();

    // Claim and complete first block
    let block = db
        .claim_work_block(job_id, "block-worker")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(block.block_start, 1);
    assert_eq!(block.block_end, 11);

    db.complete_work_block(block.block_id, 10, 2).await.unwrap();

    let summary = db.get_job_block_summary(job_id).await.unwrap();
    assert_eq!(summary.available, 2); // 2 remaining
    assert_eq!(summary.completed, 1);
    assert_eq!(summary.total_tested, 10);
    assert_eq!(summary.total_found, 2);
}

// --- Agent management ---

#[tokio::test]
async fn agent_task_lifecycle() {
    require_db!();
    let db = setup().await;

    // Create a task
    let task = db
        .create_agent_task(
            "Test task",
            "Do something",
            "normal",
            Some("opus"),
            "manual",
            None,
            1,
            None,
        )
        .await
        .unwrap();
    assert_eq!(task.title, "Test task");
    assert_eq!(task.status, "pending");
    assert_eq!(task.priority, "normal");

    // Retrieve by ID
    let fetched = db.get_agent_task(task.id).await.unwrap().unwrap();
    assert_eq!(fetched.title, "Test task");

    // Update status to in_progress
    db.update_agent_task_status(task.id, "in_progress")
        .await
        .unwrap();
    let fetched = db.get_agent_task(task.id).await.unwrap().unwrap();
    assert_eq!(fetched.status, "in_progress");
    assert!(fetched.started_at.is_some());

    // Cancel it
    db.cancel_agent_task(task.id).await.unwrap();
    let fetched = db.get_agent_task(task.id).await.unwrap().unwrap();
    assert_eq!(fetched.status, "cancelled");
    assert!(fetched.completed_at.is_some());

    // List tasks by status
    let tasks = db.get_agent_tasks(Some("cancelled"), 10).await.unwrap();
    assert!(tasks.iter().any(|t| t.id == task.id));
}

// --- Agent task decomposition ---

#[tokio::test]
async fn template_expand_creates_parent_and_children() {
    require_db!();
    let db = setup().await;

    let parent_id = db
        .expand_template(
            "fix-bug",
            "Fix login bug",
            "Users can't log in",
            "high",
            Some(5.0),
            1,
            None,
        )
        .await
        .unwrap();

    // Parent task exists
    let parent = db.get_agent_task(parent_id).await.unwrap().unwrap();
    assert_eq!(parent.title, "Fix login bug");
    assert_eq!(parent.priority, "high");
    assert_eq!(parent.template_name.as_deref(), Some("fix-bug"));
    assert_eq!(parent.on_child_failure, "fail");
    assert_eq!(parent.status, "pending");

    // Children exist
    let children = db.get_child_tasks(parent_id).await.unwrap();
    assert_eq!(children.len(), 3, "fix-bug template has 3 steps");
    for child in &children {
        assert_eq!(child.parent_task_id, Some(parent_id));
        assert_eq!(child.priority, "high");
        assert_eq!(child.source, "automated");
    }
    assert!(children[0].title.contains("Investigate"));
    assert!(children[1].title.contains("Fix"));
    assert!(children[2].title.contains("Verify"));
}

#[tokio::test]
async fn template_expand_creates_dependencies() {
    require_db!();
    let db = setup().await;

    let parent_id = db
        .expand_template("fix-bug", "Test deps", "test", "normal", None, 2, None)
        .await
        .unwrap();

    let children = db.get_child_tasks(parent_id).await.unwrap();
    assert_eq!(children.len(), 3);

    // Step 0 (Investigate) has no deps
    let deps0 = db.get_task_deps(children[0].id).await.unwrap();
    assert!(deps0.is_empty());

    // Step 1 (Fix) depends on step 0 (Investigate)
    let deps1 = db.get_task_deps(children[1].id).await.unwrap();
    assert_eq!(deps1, vec![children[0].id]);

    // Step 2 (Verify) depends on step 1 (Fix)
    let deps2 = db.get_task_deps(children[2].id).await.unwrap();
    assert_eq!(deps2, vec![children[1].id]);
}

#[tokio::test]
async fn claim_skips_tasks_with_unmet_deps() {
    require_db!();
    let db = setup().await;

    let parent_id = db
        .expand_template("fix-bug", "Dep test", "test", "normal", None, 1, None)
        .await
        .unwrap();

    let children = db.get_child_tasks(parent_id).await.unwrap();
    // Only step 0 should be claimable (no deps)
    let claimed = db.claim_pending_agent_task("test-agent").await.unwrap();
    assert!(claimed.is_some());
    let claimed = claimed.unwrap();
    assert_eq!(claimed.id, children[0].id, "Should claim step 0 (no deps)");

    // Step 1 depends on step 0 which is now in_progress, not completed
    // So nothing more should be claimable
    let next = db.claim_pending_agent_task("test-agent").await.unwrap();
    assert!(
        next.is_none(),
        "Step 1 should not be claimable while step 0 is in_progress"
    );
}

#[tokio::test]
async fn claim_skips_parent_tasks() {
    require_db!();
    let db = setup().await;

    // Create a standalone task and a template task
    db.create_agent_task(
        "Standalone",
        "solo task",
        "normal",
        None,
        "manual",
        None,
        1,
        None,
    )
    .await
    .unwrap();
    let _parent_id = db
        .expand_template("fix-bug", "Parent", "test", "urgent", None, 1, None)
        .await
        .unwrap();

    // The parent task has children, so it should be skipped.
    // With urgent priority the parent would be first, but it should be skipped.
    // The first claimable should be the standalone task or the first child.
    let claimed = db
        .claim_pending_agent_task("test-agent")
        .await
        .unwrap()
        .unwrap();
    // Parent should never be claimed (it has children)
    assert_ne!(
        claimed.title, "Parent",
        "Parent tasks should not be claimed directly"
    );
}

#[tokio::test]
async fn try_complete_parent_succeeds() {
    require_db!();
    let db = setup().await;

    let parent_id = db
        .expand_template("fix-bug", "Complete test", "test", "normal", None, 1, None)
        .await
        .unwrap();

    let children = db.get_child_tasks(parent_id).await.unwrap();

    // Complete all children
    for child in &children {
        db.complete_agent_task(child.id, "completed", None, 100, 0.01)
            .await
            .unwrap();
    }

    // Now try_complete_parent should succeed
    let result = db.try_complete_parent(parent_id).await.unwrap();
    assert!(result.is_some(), "Parent should be auto-completed");
    let parent = result.unwrap();
    assert_eq!(parent.status, "completed");
}

#[tokio::test]
async fn try_complete_parent_fails_on_child_failure() {
    require_db!();
    let db = setup().await;

    let parent_id = db
        .expand_template("fix-bug", "Fail test", "test", "normal", None, 1, None)
        .await
        .unwrap();

    let children = db.get_child_tasks(parent_id).await.unwrap();

    // Complete first child, fail second, cancel third
    db.complete_agent_task(children[0].id, "completed", None, 100, 0.01)
        .await
        .unwrap();
    db.complete_agent_task(
        children[1].id,
        "failed",
        Some(&serde_json::json!({"error": "test"})),
        50,
        0.005,
    )
    .await
    .unwrap();
    db.cancel_agent_task(children[2].id).await.unwrap();

    // Parent should be marked as failed (on_child_failure = 'fail')
    let result = db.try_complete_parent(parent_id).await.unwrap();
    assert!(result.is_some());
    let parent = result.unwrap();
    assert_eq!(parent.status, "failed");
}

#[tokio::test]
async fn permission_inheritance() {
    require_db!();
    let db = setup().await;

    // Parent has permission_level 1, template steps request 0 and 1
    let parent_id = db
        .expand_template("fix-bug", "Perm test", "test", "normal", None, 1, None)
        .await
        .unwrap();

    let children = db.get_child_tasks(parent_id).await.unwrap();
    // Step 0 requests level 0, parent level is 1 → min(0, 1) = 0
    assert_eq!(children[0].permission_level, 0);
    // Step 1 requests level 1, parent level is 1 → min(1, 1) = 1
    assert_eq!(children[1].permission_level, 1);
    // Step 2 requests level 1, parent level is 1 → min(1, 1) = 1
    assert_eq!(children[2].permission_level, 1);

    // Now test with a more restrictive parent (level 0)
    let parent_id_0 = db
        .expand_template("fix-bug", "Perm test 0", "test", "normal", None, 0, None)
        .await
        .unwrap();

    let children_0 = db.get_child_tasks(parent_id_0).await.unwrap();
    // All children should be capped at 0
    for child in &children_0 {
        assert_eq!(
            child.permission_level, 0,
            "Child permission should be capped at parent level 0"
        );
    }
}

// --- Agent memory ---

#[tokio::test]
async fn agent_memory_upsert_and_retrieve() {
    require_db!();
    let db = setup().await;

    let entry = db
        .upsert_agent_memory("test_key", "test_value", "pattern", None)
        .await
        .unwrap();
    assert_eq!(entry.key, "test_key");
    assert_eq!(entry.value, "test_value");
    assert_eq!(entry.category, "pattern");

    let all = db.get_all_agent_memory().await.unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].key, "test_key");

    let by_cat = db.get_agent_memory_by_category("pattern").await.unwrap();
    assert_eq!(by_cat.len(), 1);

    let empty = db.get_agent_memory_by_category("gotcha").await.unwrap();
    assert!(empty.is_empty());
}

#[tokio::test]
async fn agent_memory_upsert_updates_existing() {
    require_db!();
    let db = setup().await;

    db.upsert_agent_memory("key1", "original", "general", None)
        .await
        .unwrap();

    let updated = db
        .upsert_agent_memory("key1", "updated_value", "gotcha", None)
        .await
        .unwrap();
    assert_eq!(updated.value, "updated_value");
    assert_eq!(updated.category, "gotcha");

    // Still only one entry
    let all = db.get_all_agent_memory().await.unwrap();
    assert_eq!(all.len(), 1);
}

#[tokio::test]
async fn agent_memory_delete() {
    require_db!();
    let db = setup().await;

    db.upsert_agent_memory("to_delete", "value", "general", None)
        .await
        .unwrap();

    let deleted = db.delete_agent_memory("to_delete").await.unwrap();
    assert!(deleted);

    let not_found = db.delete_agent_memory("to_delete").await.unwrap();
    assert!(!not_found);

    let all = db.get_all_agent_memory().await.unwrap();
    assert!(all.is_empty());
}

// --- Agent roles ---

#[tokio::test]
async fn role_crud_lifecycle() {
    require_db!();
    let db = setup().await;

    // get_all_roles returns the 4 seeded roles
    let roles = db.get_all_roles().await.unwrap();
    assert_eq!(roles.len(), 4, "Should have 4 seeded roles");

    let names: Vec<&str> = roles.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"engine"));
    assert!(names.contains(&"frontend"));
    assert!(names.contains(&"ops"));
    assert!(names.contains(&"research"));

    // Verify engine role fields
    let engine = roles.iter().find(|r| r.name == "engine").unwrap();
    assert_eq!(engine.default_permission_level, 2);
    assert_eq!(engine.default_model, "sonnet");
    assert!(engine.system_prompt.is_some());
    assert_eq!(engine.default_max_cost_usd, Some(5.0));

    // Verify research role defaults
    let research = roles.iter().find(|r| r.name == "research").unwrap();
    assert_eq!(research.default_permission_level, 0);
    assert_eq!(research.default_model, "haiku");
    assert_eq!(research.default_max_cost_usd, Some(1.0));
}

#[tokio::test]
async fn role_get_by_name() {
    require_db!();
    let db = setup().await;

    let role = db.get_role_by_name("ops").await.unwrap();
    assert!(role.is_some());
    let role = role.unwrap();
    assert_eq!(role.name, "ops");
    assert_eq!(role.default_permission_level, 3);

    // Non-existent role
    let missing = db.get_role_by_name("nonexistent").await.unwrap();
    assert!(missing.is_none());
}

#[tokio::test]
async fn role_templates_association() {
    require_db!();
    let db = setup().await;

    // Engine role should have fix-bug template (seeded in test helper)
    let templates = db.get_role_templates("engine").await.unwrap();
    let names: Vec<&str> = templates.iter().map(|t| t.name.as_str()).collect();
    assert!(
        names.contains(&"fix-bug"),
        "Engine role should have fix-bug template"
    );

    // Frontend role should also have fix-bug
    let frontend_templates = db.get_role_templates("frontend").await.unwrap();
    let frontend_names: Vec<&str> = frontend_templates.iter().map(|t| t.name.as_str()).collect();
    assert!(
        frontend_names.contains(&"fix-bug"),
        "Frontend role should have fix-bug template"
    );

    // Research role has no templates seeded in test helper
    let research_templates = db.get_role_templates("research").await.unwrap();
    assert!(
        research_templates.is_empty(),
        "Research role has no templates in test seed"
    );
}

#[tokio::test]
async fn task_with_role_stores_role_name() {
    require_db!();
    let db = setup().await;

    // Create task with engine role
    let task = db
        .create_agent_task(
            "Optimize sieve",
            "Improve Montgomery multiplication performance",
            "normal",
            None,
            "manual",
            None,
            2,
            Some("engine"),
        )
        .await
        .unwrap();

    assert_eq!(task.role_name.as_deref(), Some("engine"));

    // Retrieve and verify
    let fetched = db.get_agent_task(task.id).await.unwrap().unwrap();
    assert_eq!(fetched.role_name.as_deref(), Some("engine"));

    // Create task without role
    let task2 = db
        .create_agent_task(
            "Generic task",
            "test",
            "normal",
            None,
            "manual",
            None,
            1,
            None,
        )
        .await
        .unwrap();
    assert!(task2.role_name.is_none());
}

#[tokio::test]
async fn expand_template_with_role() {
    require_db!();
    let db = setup().await;

    let parent_id = db
        .expand_template(
            "fix-bug",
            "Fix engine bug",
            "Sieve returns wrong results",
            "high",
            Some(5.0),
            2,
            Some("engine"),
        )
        .await
        .unwrap();

    // Parent should have role_name set
    let parent = db.get_agent_task(parent_id).await.unwrap().unwrap();
    assert_eq!(parent.role_name.as_deref(), Some("engine"));

    // Children should also have role_name set
    let children = db.get_child_tasks(parent_id).await.unwrap();
    assert_eq!(children.len(), 3);
    for child in &children {
        assert_eq!(
            child.role_name.as_deref(),
            Some("engine"),
            "Child task should inherit role_name"
        );
    }
}

// --- Agent Observability ---

#[tokio::test]
async fn agent_log_insert_and_query() {
    require_db!();
    let db = setup().await;

    let task = db
        .create_agent_task(
            "Log test task",
            "Testing logs",
            "normal",
            None,
            "test",
            None,
            1,
            None,
        )
        .await
        .unwrap();

    // Insert 10 log lines: 7 stdout, 3 stderr
    let mut entries: Vec<(String, i32, Option<String>, String)> = Vec::new();
    for i in 1..=7 {
        entries.push((
            "stdout".into(),
            i,
            Some("assistant".into()),
            format!("stdout line {}", i),
        ));
    }
    for i in 1..=3 {
        entries.push(("stderr".into(), i, None, format!("stderr line {}", i)));
    }
    db.insert_agent_logs_batch(task.id, &entries).await.unwrap();

    // Query all logs
    let all = db.get_agent_logs(task.id, None, 0, 100).await.unwrap();
    assert_eq!(all.len(), 10);

    // Query stdout only
    let stdout_only = db
        .get_agent_logs(task.id, Some("stdout"), 0, 100)
        .await
        .unwrap();
    assert_eq!(stdout_only.len(), 7);
    assert!(stdout_only.iter().all(|l| l.stream == "stdout"));

    // Query stderr only
    let stderr_only = db
        .get_agent_logs(task.id, Some("stderr"), 0, 100)
        .await
        .unwrap();
    assert_eq!(stderr_only.len(), 3);

    // Test offset/limit pagination
    let page = db.get_agent_logs(task.id, None, 5, 3).await.unwrap();
    assert_eq!(page.len(), 3);

    // Test count
    let count = db.get_agent_log_count(task.id).await.unwrap();
    assert_eq!(count, 10);
}

#[tokio::test]
async fn agent_event_extended_fields() {
    require_db!();
    let db = setup().await;

    let task = db
        .create_agent_task(
            "Event ext test",
            "Testing extended events",
            "normal",
            None,
            "test",
            None,
            1,
            None,
        )
        .await
        .unwrap();

    db.insert_agent_event_ex(
        Some(task.id),
        "tool_call",
        Some("claude"),
        "Tool call: Read",
        None,
        Some("Read"),
        Some(1500),
        Some(300),
        Some(250),
    )
    .await
    .unwrap();

    let events = db.get_agent_events(Some(task.id), 10).await.unwrap();
    assert_eq!(events.len(), 1);
    let ev = &events[0];
    assert_eq!(ev.tool_name.as_deref(), Some("Read"));
    assert_eq!(ev.input_tokens, Some(1500));
    assert_eq!(ev.output_tokens, Some(300));
    assert_eq!(ev.duration_ms, Some(250));

    // Timeline also returns extended fields
    let timeline = db.get_agent_task_timeline(task.id).await.unwrap();
    assert_eq!(timeline.len(), 1);
    assert_eq!(timeline[0].tool_name.as_deref(), Some("Read"));
}

#[tokio::test]
async fn agent_daily_cost_analytics() {
    require_db!();
    let db = setup().await;

    for (model, cost, tokens) in [
        ("sonnet", 0.05, 5000i64),
        ("sonnet", 0.03, 3000),
        ("opus", 0.15, 8000),
    ] {
        let task = db
            .create_agent_task(
                "Cost test",
                "",
                "normal",
                Some(model),
                "test",
                None,
                1,
                None,
            )
            .await
            .unwrap();
        let result = serde_json::json!({"text": "done"});
        db.complete_agent_task(task.id, "completed", Some(&result), tokens, cost)
            .await
            .unwrap();
    }

    let daily = db.get_agent_daily_costs(30).await.unwrap();
    assert!(!daily.is_empty());
    let total_cost: f64 = daily.iter().map(|r| r.total_cost).sum();
    assert!((total_cost - 0.23).abs() < 0.01);
}

#[tokio::test]
async fn agent_token_anomaly_detection() {
    require_db!();
    let db = setup().await;

    sqlx::raw_sql(
        "INSERT INTO agent_templates (name, description, steps) VALUES
          ('anomaly-test', 'Test template', '[]'::jsonb)
         ON CONFLICT (name) DO NOTHING",
    )
    .execute(db.pool())
    .await
    .unwrap();

    // 4 normal tasks (1000 tokens), 1 outlier (10000 tokens)
    for tokens in [1000i64, 1000, 1000, 1000, 10000] {
        let task = db
            .create_agent_task(
                "Anomaly test",
                "",
                "normal",
                Some("sonnet"),
                "test",
                None,
                1,
                None,
            )
            .await
            .unwrap();
        sqlx::query("UPDATE agent_tasks SET template_name = 'anomaly-test' WHERE id = $1")
            .bind(task.id)
            .execute(db.pool())
            .await
            .unwrap();
        let result = serde_json::json!({"text": "done"});
        db.complete_agent_task(task.id, "completed", Some(&result), tokens, 0.01)
            .await
            .unwrap();
    }

    let anomalies = db.get_agent_token_anomalies(2.0).await.unwrap();
    assert!(!anomalies.is_empty());
    assert!(anomalies.iter().all(|t| t.tokens_used > 5000));
}
