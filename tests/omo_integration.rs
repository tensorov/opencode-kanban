//! Integration tests for the `omo` module — FsPlanReader + OmoAdapter.
//!
//! Uses `tempfile::TempDir` to create throwaway `.omo/plans/` directories
//! and exercises the full lifecycle: writing plan files → discovering them →
//! loading into the adapter → querying cards and detail → status overrides.

#![allow(dead_code)]

use std::path::Path;

use opencode_kanban::omo::adapter::OmoAdapter;
use opencode_kanban::omo::fs_reader::FsPlanReader;
use opencode_kanban::omo::notepad::{discover_notepads, plan_to_notepad};
use opencode_kanban::omo::reader::PlanReader;
use opencode_kanban::omo::types::{OmoError, PlanStatus};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a minimal plan file at `{root}/plans/{slug}.md`.
/// Generates `- [x]` for the first `done` items, `- [ ]` for the rest.
fn create_plan_file(root: &Path, slug: &str, title: &str, done: usize, total: usize) {
    let plans_dir = root.join("plans");
    std::fs::create_dir_all(&plans_dir).unwrap();

    let mut content = format!("# {title} - Work Plan\n\n");
    for i in 0..total {
        let marker = if i < done { "x" } else { " " };
        content.push_str(&format!("- [{marker}] item {}\n", i + 1));
    }

    std::fs::write(plans_dir.join(format!("{slug}.md")), &content).unwrap();
}

/// Create a rich plan file with TL;DR, scope, and checklist sections.
fn create_detailed_plan_file(root: &Path, slug: &str) {
    let plans_dir = root.join("plans");
    std::fs::create_dir_all(&plans_dir).unwrap();

    let content = "\
# Feature Plan - Work Plan

Some preamble text.

## TL;DR

**What you'll get:** A full-featured integration
**Why this approach:** Minimal changes to existing code
**Effort:** Medium
**Risk:** Low

## Scope

### Must have
- Core engine
- API layer
- Tests

### Must NOT have
- Database migration
- Admin panel

## Tasks

- [ ] implement core
- [x] design API
- [x] write unit tests
- [ ] review code
  - [ ] fix nits
";
    std::fs::write(plans_dir.join(format!("{slug}.md")), content).unwrap();
}

// ---------------------------------------------------------------------------
// FsPlanReader integration tests
// ---------------------------------------------------------------------------

#[test]
fn test_fs_reader_discovers_plans() {
    let tmp = TempDir::new().unwrap();
    create_plan_file(tmp.path(), "alpha", "Alpha", 1, 2);
    create_plan_file(tmp.path(), "beta", "Beta", 0, 1);

    let reader = FsPlanReader::new(tmp.path().to_path_buf());
    let plans = reader.list_plans().unwrap();

    assert_eq!(plans.len(), 2);

    let alpha = plans.iter().find(|p| p.slug == "alpha").unwrap();
    assert_eq!(alpha.title, "Alpha");
    assert_eq!(alpha.checklist.len(), 2);
    assert!(alpha.checklist[0].done);
    assert!(!alpha.checklist[1].done);

    let beta = plans.iter().find(|p| p.slug == "beta").unwrap();
    assert_eq!(beta.title, "Beta");
    assert_eq!(beta.checklist.len(), 1);
    assert!(!beta.checklist[0].done);
}

#[test]
fn test_fs_reader_reads_single_plan() {
    let tmp = TempDir::new().unwrap();
    create_detailed_plan_file(tmp.path(), "feature-x");

    let reader = FsPlanReader::new(tmp.path().to_path_buf());
    let plan = reader.read_plan("feature-x").unwrap();

    assert_eq!(plan.slug, "feature-x");
    assert_eq!(plan.title, "Feature Plan");
    assert!(plan.tl_dr.is_some());
    assert!(plan.tl_dr.as_deref().unwrap().contains("What you'll get:"));
    assert_eq!(plan.scope_in, vec!["Core engine", "API layer", "Tests"]);
    assert_eq!(plan.scope_out, vec!["Database migration", "Admin panel"]);
    assert_eq!(plan.checklist.len(), 5);
    assert_eq!(plan.checklist[4].text, "fix nits");
    assert_eq!(plan.checklist[4].level, 1);
}

#[test]
fn test_fs_reader_reads_nonexistent_plan() {
    let tmp = TempDir::new().unwrap();
    let reader = FsPlanReader::new(tmp.path().to_path_buf());
    let result = reader.read_plan("ghost");
    assert!(matches!(result, Err(OmoError::NotFound)));
}

#[test]
fn test_fs_reader_empty_plans_dir() {
    let tmp = TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("plans")).unwrap();

    let reader = FsPlanReader::new(tmp.path().to_path_buf());
    let plans = reader.list_plans().unwrap();
    assert!(plans.is_empty());
}

#[test]
fn test_fs_reader_no_plans_dir() {
    let tmp = TempDir::new().unwrap();
    let reader = FsPlanReader::new(tmp.path().to_path_buf());
    let plans = reader.list_plans().unwrap();
    assert!(plans.is_empty());
}

// ---------------------------------------------------------------------------
// OmoAdapter integration tests
// ---------------------------------------------------------------------------

#[test]
fn test_adapter_load_plans_and_get_plans() {
    let tmp = TempDir::new().unwrap();
    create_plan_file(tmp.path(), "plan-a", "Plan A", 2, 3);
    create_plan_file(tmp.path(), "plan-b", "Plan B", 1, 1);

    let reader = FsPlanReader::new(tmp.path().to_path_buf());
    let mut adapter = OmoAdapter::new(Box::new(reader));
    adapter.load_plans();

    let cards = adapter.get_plans();
    assert_eq!(cards.len(), 2);

    let a = cards.iter().find(|c| c.slug == "plan-a").unwrap();
    assert_eq!(a.title, "Plan A");
    assert_eq!(a.checklist_total, 3);
    assert_eq!(a.checklist_done, 2);

    let b = cards.iter().find(|c| c.slug == "plan-b").unwrap();
    assert_eq!(b.title, "Plan B");
    assert_eq!(b.checklist_total, 1);
    assert_eq!(b.checklist_done, 1);
}

#[test]
fn test_adapter_get_plan_detail_returns_full_plan() {
    let tmp = TempDir::new().unwrap();
    create_detailed_plan_file(tmp.path(), "detailed");

    let reader = FsPlanReader::new(tmp.path().to_path_buf());
    let mut adapter = OmoAdapter::new(Box::new(reader));
    adapter.load_plans();

    let detail = adapter.get_plan_detail("detailed").unwrap();
    assert_eq!(detail.slug, "detailed");
    assert_eq!(detail.title, "Feature Plan");
    assert!(detail.tl_dr.is_some());
    assert_eq!(detail.scope_in, vec!["Core engine", "API layer", "Tests"]);
    assert_eq!(detail.scope_out, vec!["Database migration", "Admin panel"]);
    assert_eq!(detail.checklist.len(), 5);
    assert!(detail.checklist[1].done);
    assert!(!detail.checklist[3].done);

    // Unknown slug returns None
    assert!(adapter.get_plan_detail("nonexistent").is_none());
}

#[test]
fn test_adapter_plan_status_default_and_override() {
    let tmp = TempDir::new().unwrap();
    create_plan_file(tmp.path(), "my-plan", "My Plan", 0, 1);

    let reader = FsPlanReader::new(tmp.path().to_path_buf());
    let mut adapter = OmoAdapter::new(Box::new(reader));
    adapter.load_plans();

    // Parser always sets Drafting
    assert_eq!(adapter.plan_status("my-plan"), PlanStatus::Drafting);

    // Override to Active
    adapter.set_plan_status("my-plan", PlanStatus::Active);
    assert_eq!(adapter.plan_status("my-plan"), PlanStatus::Active);

    // Override to Completed
    adapter.set_plan_status("my-plan", PlanStatus::Completed);
    assert_eq!(adapter.plan_status("my-plan"), PlanStatus::Completed);

    // Status override survives load_plan
    adapter.load_plan("my-plan");
    assert_eq!(adapter.plan_status("my-plan"), PlanStatus::Completed);
}

#[test]
fn test_adapter_plan_status_fallback_to_drafting() {
    let tmp = TempDir::new().unwrap();
    // Create a plans dir so FsPlanReader doesn't error.
    std::fs::create_dir_all(tmp.path().join("plans")).unwrap();
    let reader = FsPlanReader::new(tmp.path().to_path_buf());
    let adapter = OmoAdapter::new(Box::new(reader));
    assert_eq!(adapter.plan_status("unknown"), PlanStatus::Drafting);
}

#[test]
fn test_adapter_empty_dir_returns_no_cards() {
    let tmp = TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("plans")).unwrap();

    let reader = FsPlanReader::new(tmp.path().to_path_buf());
    let mut adapter = OmoAdapter::new(Box::new(reader));
    adapter.load_plans();
    assert!(adapter.get_plans().is_empty());
}

#[test]
fn test_adapter_lazy_load_plan() {
    let tmp = TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("plans")).unwrap();

    let reader = FsPlanReader::new(tmp.path().to_path_buf());
    let mut adapter = OmoAdapter::new(Box::new(reader));
    adapter.load_plans();
    assert!(adapter.get_plans().is_empty());
    assert!(adapter.get_plan_detail("lazy").is_none());

    // Now add a plan file and lazy-load it.
    create_plan_file(tmp.path(), "lazy", "Lazy Loaded", 1, 3);
    adapter.load_plan("lazy");

    assert_eq!(adapter.get_plans().len(), 1);
    let card = &adapter.get_plans()[0];
    assert_eq!(card.slug, "lazy");
    assert_eq!(card.checklist_total, 3);
    assert_eq!(card.checklist_done, 1);

    let detail = adapter.get_plan_detail("lazy").unwrap();
    assert_eq!(detail.title, "Lazy Loaded");
    assert_eq!(detail.checklist.len(), 3);
}

#[test]
fn test_adapter_load_many_plans() {
    let tmp = TempDir::new().unwrap();
    // Create more than a handful of plans to verify batch handling.
    for i in 0..10 {
        let slug = format!("plan-{i}");
        let title = format!("Plan {i}");
        create_plan_file(tmp.path(), &slug, &title, i % 3, 4);
    }

    let reader = FsPlanReader::new(tmp.path().to_path_buf());
    let mut adapter = OmoAdapter::new(Box::new(reader));
    adapter.load_plans();

    assert_eq!(adapter.get_plans().len(), 10);

    for i in 0..10 {
        let slug = format!("plan-{i}");
        let card = adapter
            .get_plans()
            .iter()
            .find(|c| c.slug == slug)
            .unwrap_or_else(|| panic!("card {slug} not found"));
        assert_eq!(card.checklist_total, 4);
        assert_eq!(card.checklist_done, i % 3);
    }
}

// ---------------------------------------------------------------------------
// Full integration: write → discover → load → query → status override
// ---------------------------------------------------------------------------

#[test]
fn test_full_plan_lifecycle_via_adapter() {
    let tmp = TempDir::new().unwrap();

    // 1. Write a plan file
    create_detailed_plan_file(tmp.path(), "lifecycle-plan");

    // 2. Discover via FsPlanReader (standalone)
    let reader = FsPlanReader::new(tmp.path().to_path_buf());
    let plans = reader.list_plans().unwrap();
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].slug, "lifecycle-plan");

    // 3. Load via OmoAdapter
    let reader2 = FsPlanReader::new(tmp.path().to_path_buf());
    let mut adapter = OmoAdapter::new(Box::new(reader2));
    adapter.load_plans();

    // 4. Query cards
    let cards = adapter.get_plans();
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].slug, "lifecycle-plan");
    assert_eq!(cards[0].title, "Feature Plan");
    assert_eq!(cards[0].checklist_total, 5);
    assert_eq!(cards[0].checklist_done, 2);

    // 5. Query detail
    let detail = adapter.get_plan_detail("lifecycle-plan").unwrap();
    assert_eq!(detail.title, "Feature Plan");
    assert_eq!(detail.scope_in.len(), 3);
    assert_eq!(detail.scope_out.len(), 2);
    assert!(detail.tl_dr.as_deref().unwrap().contains("Effort:"));

    // 6. Status override: Drafting -> Active
    assert_eq!(adapter.plan_status("lifecycle-plan"), PlanStatus::Drafting);
    adapter.set_plan_status("lifecycle-plan", PlanStatus::Active);
    assert_eq!(adapter.plan_status("lifecycle-plan"), PlanStatus::Active);

    // 7. Status override survives load_plan refresh
    adapter.load_plan("lifecycle-plan");
    assert_eq!(adapter.plan_status("lifecycle-plan"), PlanStatus::Active);

    // 8. Re-read plan detail (still accessible after status override)
    let detail_after = adapter.get_plan_detail("lifecycle-plan").unwrap();
    assert_eq!(detail_after.slug, "lifecycle-plan");
    assert_eq!(detail_after.checklist.len(), 5);

    // 9. Notepad discovery and plan-to-notepad mapping
    let notepads_dir = tmp.path().join("notepads").join("lifecycle-plan");
    std::fs::create_dir_all(&notepads_dir).unwrap();
    std::fs::write(notepads_dir.join("learnings.md"), "# learnings\n\nSome progress notes.").unwrap();

    let notepads = discover_notepads(tmp.path());
    assert_eq!(notepads.len(), 1, "should discover one notepad");
    assert_eq!(notepads[0].project, "lifecycle-plan");

    let matched = plan_to_notepad("lifecycle-plan", &notepads);
    assert!(matched.is_some(), "plan_to_notepad should find a match");
    assert_eq!(matched.unwrap().project, "lifecycle-plan");

    // 10. Status transition to Completed
    adapter.set_plan_status("lifecycle-plan", PlanStatus::Completed);
    assert_eq!(adapter.plan_status("lifecycle-plan"), PlanStatus::Completed);

    // 11. Verify card reflects updated status after reload
    adapter.load_plans(); // reload cards from filesystem
    let refreshed = adapter
        .get_plans()
        .iter()
        .find(|c| c.slug == "lifecycle-plan")
        .unwrap();
    assert_eq!(refreshed.slug, "lifecycle-plan");
    // Card checklist (from file) is unchanged
    assert_eq!(refreshed.checklist_total, 5);
    assert_eq!(refreshed.checklist_done, 2);
    // Adapter-level status override persists
    assert_eq!(adapter.plan_status("lifecycle-plan"), PlanStatus::Completed);
}
