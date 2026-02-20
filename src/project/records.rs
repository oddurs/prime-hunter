//! World record tracking via t5k.org (The Prime Pages) scraping.
//!
//! Periodically fetches the Top 20 lists for each prime form to track current
//! world records. Records are stored in PostgreSQL and compared against our
//! best primes to measure competitive standing.

use crate::db::Database;
use anyhow::Result;
use serde::Serialize;

/// Known t5k.org Top 20 page IDs for each form.
/// Used by `fetch_t5k_record` to scrape current world records.
pub fn t5k_page_id(form: &str) -> Option<u32> {
    match form {
        "factorial" => Some(15),
        "primorial" => Some(41),
        "wagstaff" => Some(67),
        "palindromic" => Some(39),
        "twin" => Some(1),
        "sophie_germain" => Some(2),
        "repunit" => Some(44),
        "gen_fermat" => Some(16),
        _ => None,
    }
}

/// OEIS sequence IDs for each form.
pub fn oeis_sequence(form: &str) -> Option<&'static str> {
    match form {
        "factorial" => Some("A002981"),
        "primorial" => Some("A014545"),
        "wagstaff" => Some("A000978"),
        "palindromic" => Some("A002385"),
        "sophie_germain" => Some("A005384"),
        "repunit" => Some("A004023"),
        "gen_fermat" => Some("A019434"),
        _ => None,
    }
}

/// Fetch the current world record for a form from t5k.org (The Prime Pages).
/// Parses the first table row from the Top 20 page.
pub async fn fetch_t5k_record(form: &str) -> Result<Option<RecordInfo>> {
    let page_id = match t5k_page_id(form) {
        Some(id) => id,
        None => return Ok(None),
    };

    let url = format!("https://t5k.org/top20/page.php?id={}", page_id);
    let response = reqwest::get(&url).await?;
    let html = response.text().await?;

    parse_t5k_html(&html, form)
}

/// Parsed record information from t5k.org.
#[derive(Debug, Clone, Serialize)]
pub struct RecordInfo {
    pub expression: String,
    pub digits: u64,
    pub holder: String,
    pub discovered_at: Option<String>,
    pub source_url: String,
}

/// Parse t5k.org Top 20 HTML to extract the first (largest) entry.
pub fn parse_t5k_html(html: &str, form: &str) -> Result<Option<RecordInfo>> {
    let document = scraper::Html::parse_document(html);
    let table_sel = scraper::Selector::parse("table.list").unwrap();
    let row_sel = scraper::Selector::parse("tr").unwrap();
    let cell_sel = scraper::Selector::parse("td").unwrap();

    let table = match document.select(&table_sel).next() {
        Some(t) => t,
        None => return Ok(None),
    };

    // Skip header row, get first data row
    let row = match table.select(&row_sel).nth(1) {
        Some(r) => r,
        None => return Ok(None),
    };

    let cells: Vec<String> = row
        .select(&cell_sel)
        .map(|c| c.text().collect::<String>().trim().to_string())
        .collect();

    // t5k.org Top 20 tables typically have columns:
    // rank | prime | digits | who | when | comment
    if cells.len() < 4 {
        return Ok(None);
    }

    let expression = cells[1].clone();
    let digits = cells[2].replace(',', "").parse::<u64>().unwrap_or(0);
    let holder = cells[3].clone();
    let discovered_at = cells.get(4).cloned();

    let page_id = t5k_page_id(form).unwrap_or(0);

    Ok(Some(RecordInfo {
        expression,
        digits,
        holder,
        discovered_at,
        source_url: format!("https://t5k.org/top20/page.php?id={}", page_id),
    }))
}

/// Refresh all records for forms that have t5k.org pages.
/// Called on dashboard startup and every 24 hours.
pub async fn refresh_all_records(db: &Database) -> Result<u32> {
    let forms = [
        "factorial",
        "primorial",
        "wagstaff",
        "palindromic",
        "twin",
        "sophie_germain",
        "repunit",
        "gen_fermat",
    ];
    let mut updated = 0u32;

    for form in &forms {
        match fetch_t5k_record(form).await {
            Ok(Some(record)) => {
                // Get our best prime for this form
                let our_best = db.get_best_prime_for_form(form).await.unwrap_or(None);
                let (our_best_id, our_best_digits) = our_best
                    .map(|p| (Some(p.id), p.digits))
                    .unwrap_or((None, 0));

                db.upsert_record(
                    form,
                    "overall",
                    &record.expression,
                    record.digits as i64,
                    Some(&record.holder),
                    record.discovered_at.as_deref(),
                    Some("t5k.org"),
                    Some(&record.source_url),
                    our_best_id,
                    our_best_digits,
                )
                .await?;
                updated += 1;
                eprintln!(
                    "Record updated: {} — {} ({} digits, by {})",
                    form, record.expression, record.digits, record.holder
                );
            }
            Ok(None) => {
                eprintln!("No t5k record found for form '{}'", form);
            }
            Err(e) => {
                eprintln!("Error fetching record for '{}': {}", form, e);
            }
        }
    }

    Ok(updated)
}
