use std::collections::HashMap;

use crate::omo::reader::PlanReader;
use crate::omo::types::{OmoPlan, PlanCard, PlanStatus};

/// Adapter between `PlanReader` (filesystem) and the kanban UI layer.
///
/// Provides type-erased access (`Box<dyn PlanReader>`) so the UI never
/// depends on `FsPlanReader` directly.  Caches plan data in memory and
/// supports in-memory status overrides without writing to disk.
pub struct OmoAdapter {
    reader: Box<dyn PlanReader>,
    cards: Vec<PlanCard>,
    plans: HashMap<String, OmoPlan>,
    statuses: HashMap<String, PlanStatus>,
}

impl OmoAdapter {
    /// Create a new adapter wrapping any `PlanReader`.
    pub fn new(reader: Box<dyn PlanReader>) -> Self {
        Self {
            reader,
            cards: Vec::new(),
            plans: HashMap::new(),
            statuses: HashMap::new(),
        }
    }

    /// Load all plans from the reader into memory.
    ///
    /// Replaces any previously cached cards, plans, and status overrides.
    /// Each plan is converted to a `PlanCard` and stored in the lookup map.
    pub fn load_plans(&mut self) {
        let plans = match self.reader.list_plans() {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("Failed to list plans: {}", e);
                return;
            }
        };

        self.cards.clear();
        self.plans.clear();

        for plan in plans {
            let slug = plan.slug.clone();
            let card = card_from_plan(&plan);
            self.cards.push(card);
            self.plans.insert(slug, plan);
        }
    }

    /// Return a slice of all loaded plan cards.
    pub fn get_plans(&self) -> &[PlanCard] {
        &self.cards
    }

    /// Return the full `OmoPlan` for a given slug, if loaded.
    pub fn get_plan_detail(&self, slug: &str) -> Option<&OmoPlan> {
        self.plans.get(slug)
    }

    /// Override the in-memory status for a plan.
    ///
    /// Does **not** modify the filesystem.
    pub fn set_plan_status(&mut self, slug: &str, status: PlanStatus) {
        self.statuses.insert(slug.to_string(), status);
    }

    /// Return the effective status for a plan.
    ///
    /// Priority: in-memory override > plan's own status > `PlanStatus::Drafting`.
    pub fn plan_status(&self, slug: &str) -> PlanStatus {
        self.statuses
            .get(slug)
            .copied()
            .or_else(|| self.plans.get(slug).map(|p| p.status))
            .unwrap_or(PlanStatus::Drafting)
    }

    /// Lazy-load a single plan by slug.
    ///
    /// Reads the plan from the reader, updates the lookup map, and
    /// inserts or refreshes the corresponding card in the card list.
    /// Does **not** clear existing data or other plans.
    pub fn load_plan(&mut self, slug: &str) {
        let plan = match self.reader.read_plan(slug) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("Failed to read plan '{slug}': {e}");
                return;
            }
        };

        let slug_owned = plan.slug.clone();
        let card = card_from_plan(&plan);

        self.plans.insert(slug_owned, plan);

        // Update existing card or append a new one.
        if let Some(existing) = self.cards.iter_mut().find(|c| c.slug == slug) {
            *existing = card;
        } else {
            self.cards.push(card);
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert an `OmoPlan` to a `PlanCard`, computing checklist counts.
fn card_from_plan(plan: &OmoPlan) -> PlanCard {
    PlanCard {
        slug: plan.slug.clone(),
        title: plan.title.clone(),
        status: plan.status,
        checklist_total: plan.checklist.len(),
        checklist_done: plan.checklist.iter().filter(|c| c.done).count(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::omo::fs_reader::FsPlanReader;
    use tempfile::TempDir;

    // -- helpers -----------------------------------------------------------

    /// Create a minimal plan file at `{dir}/plans/{slug}.md` with a title and
    /// a given number of checklist items (first `done` items are marked done).
    fn create_plan_file(
        dir: &std::path::Path,
        slug: &str,
        title: &str,
        done: usize,
        total: usize,
    ) {
        let plans_dir = dir.join("plans");
        std::fs::create_dir_all(&plans_dir).unwrap();
        let mut content = format!("# {title} - Work Plan\n\n");
        for i in 0..total {
            let marker = if i < done { "x" } else { " " };
            content.push_str(&format!("- [{marker}] item {}\n", i + 1));
        }
        std::fs::write(plans_dir.join(format!("{slug}.md")), &content).unwrap();
    }

    /// Create a richer plan file that includes scope and TL;DR sections.
    fn create_detailed_plan_file(dir: &std::path::Path, slug: &str) {
        let plans_dir = dir.join("plans");
        std::fs::create_dir_all(&plans_dir).unwrap();
        let content = "\
# Detailed Plan - Work Plan

## TL;DR
**What you'll get:** A rich test plan
**Effort:** Short

## Scope
### Must have
- Core feature

### Must NOT have
- Extra scope

## Tasks
- [ ] item one
- [x] item two
";
        std::fs::write(plans_dir.join(format!("{slug}.md")), content).unwrap();
    }

    // -- load_plans / get_plans -------------------------------------------

    #[test]
    fn test_load_plans_returns_three_cards() {
        let tmp = TempDir::new().unwrap();
        create_plan_file(tmp.path(), "plan-a", "Plan A", 2, 3);
        create_plan_file(tmp.path(), "plan-b", "Plan B", 0, 1);
        create_plan_file(tmp.path(), "plan-c", "Plan C", 1, 1);

        let reader = FsPlanReader::new(tmp.path().to_path_buf());
        let mut adapter = OmoAdapter::new(Box::new(reader));
        adapter.load_plans();

        let cards = adapter.get_plans();
        assert_eq!(cards.len(), 3);

        // Plan A: 3 total, 2 done
        let a = cards.iter().find(|c| c.slug == "plan-a").unwrap();
        assert_eq!(a.checklist_total, 3);
        assert_eq!(a.checklist_done, 2);

        // Plan B: 1 total, 0 done
        let b = cards.iter().find(|c| c.slug == "plan-b").unwrap();
        assert_eq!(b.checklist_total, 1);
        assert_eq!(b.checklist_done, 0);

        // Plan C: 1 total, 1 done
        let c = cards.iter().find(|c| c.slug == "plan-c").unwrap();
        assert_eq!(c.checklist_total, 1);
        assert_eq!(c.checklist_done, 1);
    }

    // -- get_plan_detail ---------------------------------------------------

    #[test]
    fn test_get_plan_detail_returns_full_plan() {
        let tmp = TempDir::new().unwrap();
        create_detailed_plan_file(tmp.path(), "test");

        let reader = FsPlanReader::new(tmp.path().to_path_buf());
        let mut adapter = OmoAdapter::new(Box::new(reader));
        adapter.load_plans();

        let detail = adapter.get_plan_detail("test").unwrap();
        assert_eq!(detail.slug, "test");
        assert_eq!(detail.title, "Detailed Plan");
        assert!(detail.tl_dr.is_some());
        assert!(detail.tl_dr.as_deref().unwrap().contains("What you'll get:"));
        assert_eq!(detail.scope_in, vec!["Core feature"]);
        assert_eq!(detail.scope_out, vec!["Extra scope"]);
        assert_eq!(detail.checklist.len(), 2);
        assert!(!detail.checklist[0].done);
        assert!(detail.checklist[1].done);
    }

    // -- set_plan_status / plan_status -------------------------------------

    #[test]
    fn test_set_plan_status_and_query() {
        let tmp = TempDir::new().unwrap();
        create_plan_file(tmp.path(), "test", "Test", 0, 1);

        let reader = FsPlanReader::new(tmp.path().to_path_buf());
        let mut adapter = OmoAdapter::new(Box::new(reader));
        adapter.load_plans();

        // Default is Drafting (from parser)
        assert_eq!(adapter.plan_status("test"), PlanStatus::Drafting);

        // Override to Active
        adapter.set_plan_status("test", PlanStatus::Active);
        assert_eq!(adapter.plan_status("test"), PlanStatus::Active);

        // Override to Completed
        adapter.set_plan_status("test", PlanStatus::Completed);
        assert_eq!(adapter.plan_status("test"), PlanStatus::Completed);
    }

    #[test]
    fn test_plan_status_override_beats_plan_file() {
        let tmp = TempDir::new().unwrap();
        create_plan_file(tmp.path(), "my-plan", "My Plan", 1, 2);

        let reader = FsPlanReader::new(tmp.path().to_path_buf());
        let mut adapter = OmoAdapter::new(Box::new(reader));
        adapter.load_plans();

        // Parser always sets Drafting; override to Active.
        assert_eq!(adapter.plan_status("my-plan"), PlanStatus::Drafting);
        adapter.set_plan_status("my-plan", PlanStatus::Active);
        assert_eq!(adapter.plan_status("my-plan"), PlanStatus::Active);

        // load_plan() refreshes the plan but the status override survives
        // because plan_status() checks the overrides HashMap first.
        adapter.load_plan("my-plan");
        assert_eq!(
            adapter.plan_status("my-plan"),
            PlanStatus::Active,
            "status override should survive load_plan"
        );
    }

    // -- plan_status fallback ----------------------------------------------

    #[test]
    fn test_plan_status_fallback_to_drafting() {
        let adapter = OmoAdapter::new(Box::new(FsPlanReader::new(
            TempDir::new().unwrap().path().to_path_buf(),
        )));
        // No plans loaded, unknown slug → Drafting
        assert_eq!(adapter.plan_status("nonexistent"), PlanStatus::Drafting);
    }

    // -- empty directory ---------------------------------------------------

    #[test]
    fn test_empty_dir_returns_empty_cards() {
        let tmp = TempDir::new().unwrap();
        let plans_dir = tmp.path().join("plans");
        std::fs::create_dir_all(&plans_dir).unwrap();

        let reader = FsPlanReader::new(tmp.path().to_path_buf());
        let mut adapter = OmoAdapter::new(Box::new(reader));
        adapter.load_plans();

        let cards = adapter.get_plans();
        assert!(cards.is_empty());
    }

    #[test]
    fn test_no_plans_dir_returns_empty_cards() {
        let tmp = TempDir::new().unwrap();

        let reader = FsPlanReader::new(tmp.path().to_path_buf());
        let mut adapter = OmoAdapter::new(Box::new(reader));
        adapter.load_plans();

        let cards = adapter.get_plans();
        assert!(cards.is_empty());
    }

    // -- load_plan (lazy single load) --------------------------------------

    #[test]
    fn test_load_plan_adds_card_and_detail() {
        let tmp = TempDir::new().unwrap();
        // Don't create any plan files initially — load_plans returns empty.
        {
            let plans_dir = tmp.path().join("plans");
            std::fs::create_dir_all(&plans_dir).unwrap();
        }

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
    fn test_load_plan_refreshes_existing_card() {
        let tmp = TempDir::new().unwrap();
        // Create plan file with 1 item (done).
        create_plan_file(tmp.path(), "mutable", "Mutable", 1, 1);

        let reader = FsPlanReader::new(tmp.path().to_path_buf());
        let mut adapter = OmoAdapter::new(Box::new(reader));
        adapter.load_plans();

        assert_eq!(adapter.get_plans()[0].checklist_done, 1);

        // Overwrite the plan file with 2 items (0 done) and reload.
        create_plan_file(tmp.path(), "mutable", "Mutable", 0, 2);
        adapter.load_plan("mutable");

        let card = adapter.get_plans().iter().find(|c| c.slug == "mutable").unwrap();
        assert_eq!(card.checklist_total, 2);
        assert_eq!(card.checklist_done, 0);
    }
}
