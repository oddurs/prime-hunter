//! Documentation API — serves markdown docs, roadmaps, and CLAUDE.md agent files.

use axum::extract::{Path as AxumPath, Query};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub(super) struct DocSearchQuery { q: String }

#[derive(Serialize)]
struct SearchSnippet { text: String, line: usize }

#[derive(Serialize)]
struct DocSearchResult { slug: String, title: String, snippets: Vec<SearchSnippet>, category: Option<String> }

pub(super) async fn handler_api_docs_search(Query(params): Query<DocSearchQuery>) -> impl IntoResponse {
    let query = params.q.to_lowercase();
    if query.is_empty() { return Json(serde_json::json!({ "results": [] })).into_response(); }
    let docs_dir = std::path::Path::new("docs");
    if !docs_dir.exists() { return Json(serde_json::json!({ "results": [] })).into_response(); }
    let mut results = Vec::new();
    let mut check_file = |path: &std::path::Path, slug: String, category: Option<String>| {
        let content = std::fs::read_to_string(path).unwrap_or_default();
        let title = extract_title(&content, &slug);
        let mut snippets = search_file_for_snippets(&content, &query);
        if title.to_lowercase().contains(&query) || !snippets.is_empty() {
            if snippets.is_empty() { snippets.push(SearchSnippet { text: title.clone(), line: 1 }); }
            results.push(DocSearchResult { slug, title, snippets, category });
        }
    };
    if let Ok(entries) = std::fs::read_dir(docs_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "md") {
                let slug = path.file_stem().unwrap().to_string_lossy().to_string();
                check_file(&path, slug, None);
            }
        }
    }
    let roadmaps_dir = docs_dir.join("roadmaps");
    if let Ok(entries) = std::fs::read_dir(&roadmaps_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "md") {
                let stem = path.file_stem().unwrap().to_string_lossy().to_string();
                check_file(&path, format!("roadmaps/{}", stem), Some("roadmaps".into()));
            }
        }
    }
    let root_roadmap = std::path::Path::new("ROADMAP.md");
    if root_roadmap.exists() { check_file(root_roadmap, "roadmaps/index".into(), Some("roadmaps".into())); }
    for &(name, path_str, _label) in AGENT_FILES {
        let path = std::path::Path::new(path_str);
        if path.exists() { check_file(path, format!("agent/{}", name), Some("agent".into())); }
    }
    results.sort_by(|a, b| a.slug.cmp(&b.slug));
    Json(serde_json::json!({ "results": results })).into_response()
}

#[derive(Serialize)]
struct DocEntry { slug: String, title: String, form: Option<String>, category: Option<String> }

fn doc_form(slug: &str) -> Option<String> {
    match slug { "factorial" => Some("Factorial".into()), "palindromic" => Some("Palindromic".into()), "kbn" => Some("Kbn".into()), _ => None }
}

fn extract_title(content: &str, fallback: &str) -> String {
    content.lines().next().unwrap_or(fallback).trim_start_matches('#').trim().to_string()
}

fn search_file_for_snippets(content: &str, query: &str) -> Vec<SearchSnippet> {
    let mut snippets = Vec::new();
    for (i, line) in content.lines().enumerate() {
        if line.to_lowercase().contains(query) {
            let text = if line.len() > 120 {
                let lower = line.to_lowercase();
                let pos = lower.find(query).unwrap_or(0);
                let start = pos.saturating_sub(40);
                let end = (pos + query.len() + 40).min(line.len());
                let mut snippet = String::new();
                if start > 0 { snippet.push_str("..."); }
                snippet.push_str(&line[start..end]);
                if end < line.len() { snippet.push_str("..."); }
                snippet
            } else { line.to_string() };
            snippets.push(SearchSnippet { text, line: i + 1 });
            if snippets.len() >= 3 { break; }
        }
    }
    snippets
}

pub(super) async fn handler_api_doc_roadmap(AxumPath(slug): AxumPath<String>) -> impl IntoResponse {
    if slug.contains('/') || slug.contains('\\') || slug.contains("..") { return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Invalid slug"}))).into_response(); }
    let path = if slug == "index" { std::path::PathBuf::from("ROADMAP.md") } else { std::path::Path::new("docs/roadmaps").join(format!("{}.md", slug)) };
    match std::fs::read_to_string(&path) {
        Ok(content) => { let title = extract_title(&content, &slug); Json(serde_json::json!({"slug": format!("roadmaps/{}", slug), "title": title, "content": content, "category": "roadmaps"})).into_response() }
        Err(_) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Doc not found"}))).into_response(),
    }
}

pub(super) async fn handler_api_docs() -> impl IntoResponse {
    let docs_dir = std::path::Path::new("docs");
    if !docs_dir.exists() { return Json(serde_json::json!({ "docs": [] })).into_response(); }
    let mut docs = Vec::new();
    if let Ok(entries) = std::fs::read_dir(docs_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "md") {
                let slug = path.file_stem().unwrap().to_string_lossy().to_string();
                let content = std::fs::read_to_string(&path).unwrap_or_default();
                let title = extract_title(&content, &slug);
                let form = doc_form(&slug);
                docs.push(DocEntry { slug, title, form, category: None });
            }
        }
    }
    let roadmaps_dir = docs_dir.join("roadmaps");
    if let Ok(entries) = std::fs::read_dir(&roadmaps_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "md") {
                let stem = path.file_stem().unwrap().to_string_lossy().to_string();
                let content = std::fs::read_to_string(&path).unwrap_or_default();
                let title = extract_title(&content, &stem);
                docs.push(DocEntry { slug: format!("roadmaps/{}", stem), title, form: None, category: Some("roadmaps".into()) });
            }
        }
    }
    let root_roadmap = std::path::Path::new("ROADMAP.md");
    if root_roadmap.exists() {
        let content = std::fs::read_to_string(root_roadmap).unwrap_or_default();
        let title = extract_title(&content, "Roadmap");
        docs.push(DocEntry { slug: "roadmaps/index".into(), title, form: None, category: Some("roadmaps".into()) });
    }
    for &(name, path_str, label) in AGENT_FILES {
        let path = std::path::Path::new(path_str);
        if path.exists() {
            let content = std::fs::read_to_string(path).unwrap_or_default();
            let title = extract_title(&content, &format!("CLAUDE.md ({})", label));
            docs.push(DocEntry { slug: format!("agent/{}", name), title, form: None, category: Some("agent".into()) });
        }
    }
    docs.sort_by(|a, b| a.slug.cmp(&b.slug));
    Json(serde_json::json!({ "docs": docs })).into_response()
}

const AGENT_FILES: &[(&str, &str, &str)] = &[
    ("root", "CLAUDE.md", "Project"), ("engine", "src/CLAUDE.md", "Engine"),
    ("frontend", "frontend/CLAUDE.md", "Frontend"), ("docs", "docs/CLAUDE.md", "Research"),
    ("deploy", "deploy/CLAUDE.md", "Deployment"),
];

pub(super) async fn handler_api_doc_agent(AxumPath(slug): AxumPath<String>) -> impl IntoResponse {
    if slug.contains('/') || slug.contains('\\') || slug.contains("..") { return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Invalid slug"}))).into_response(); }
    for &(name, path_str, _label) in AGENT_FILES {
        if name == slug {
            let path = std::path::Path::new(path_str);
            return match std::fs::read_to_string(path) {
                Ok(content) => { let title = extract_title(&content, &format!("CLAUDE.md ({})", name)); Json(serde_json::json!({"slug": format!("agent/{}", name), "title": title, "content": content, "category": "agent"})).into_response() }
                Err(_) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Agent file not found"}))).into_response(),
            };
        }
    }
    (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Unknown agent file"}))).into_response()
}

pub(super) async fn handler_api_doc(AxumPath(slug): AxumPath<String>) -> impl IntoResponse {
    if slug.contains('/') || slug.contains('\\') || slug.contains("..") { return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Invalid slug"}))).into_response(); }
    let path = std::path::Path::new("docs").join(format!("{}.md", slug));
    match std::fs::read_to_string(&path) {
        Ok(content) => { let title = extract_title(&content, &slug); Json(serde_json::json!({"slug": slug, "title": title, "content": content})).into_response() }
        Err(_) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Doc not found"}))).into_response(),
    }
}
