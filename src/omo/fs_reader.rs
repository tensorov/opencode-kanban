use std::path::PathBuf;

use crate::omo::notepad;
use crate::omo::parser;
use crate::omo::reader::PlanReader;
use crate::omo::types::{OmoError, OmoNotepad, OmoPlan};

/// Filesystem-backed `PlanReader`.
///
/// Reads omo plan markdown files from `{root}/plans/{slug}.md`.
/// Each read hits the filesystem — no caching, no hot-reload.
pub struct FsPlanReader {
    root: PathBuf,
}

impl FsPlanReader {
    /// Create a new reader rooted at `root` (expected to contain a `plans/` subdirectory).
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Resolve the default omo home directory (`~/.omo/`) using `dirs::home_dir()`
    /// with a fallback to the `HOME` environment variable.
    ///
    /// Returns `Err(OmoError::NoOmoHome)` when neither source yields a path.
    pub fn from_omo_home() -> Result<Self, OmoError> {
        let home = dirs::home_dir()
            .or_else(|| std::env::var("HOME").ok().map(PathBuf::from))
            .ok_or(OmoError::NoOmoHome)?;
        Ok(Self {
            root: home.join(".omo"),
        })
    }

    /// Discover notepad directories under `{root}/notepads/*/learnings.md`.
    ///
    /// Delegates to [`notepad::discover_notepads`] for the real implementation.
    pub fn discover_notepads(&self) -> Vec<OmoNotepad> {
        notepad::discover_notepads(&self.root)
    }
}

impl PlanReader for FsPlanReader {
    /// List all plans under `{root}/plans/*.md`.
    ///
    /// Files that fail to parse are skipped with a `tracing::warn!` log entry.
    /// Permission errors and missing directories are handled gracefully
    /// (logged, empty vec returned).
    fn list_plans(&self) -> Result<Vec<OmoPlan>, OmoError> {
        let plans_dir = self.root.join("plans");

        let entries = match std::fs::read_dir(&plans_dir) {
            Ok(entries) => entries,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    // No plans directory at all — not an error, just empty.
                    return Ok(Vec::new());
                }
                tracing::warn!("Failed to read plans dir {:?}: {}", plans_dir, e);
                return Ok(Vec::new());
            }
        };

        let mut plans: Vec<OmoPlan> = Vec::new();

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!("Failed to read directory entry: {}", e);
                    continue;
                }
            };

            let path = entry.path();

            // Only process files with .md extension.
            if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
                continue;
            }

            // Derive the plan slug from the file stem (e.g. "my-plan" from "my-plan.md").
            let slug = match path.file_stem().and_then(|s| s.to_str()) {
                Some(s) => s.to_string(),
                None => continue,
            };

            // Atomic file read — no partial read race.
            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!("Failed to read plan file {:?}: {}", path, e);
                    continue;
                }
            };

            match parser::parse_plan(&content, &slug) {
                Ok(plan) => plans.push(plan),
                Err(e) => {
                    tracing::warn!("Failed to parse plan '{}': {}", slug, e);
                }
            }
        }

        Ok(plans)
    }

    /// Read a single plan by slug.
    ///
    /// Returns `Err(OmoError::NotFound)` if the file does not exist,
    /// `Err(OmoError::Io(...))` on other I/O errors, and
    /// `Err(OmoError::Parse(...))` if the content cannot be parsed.
    fn read_plan(&self, slug: &str) -> Result<OmoPlan, OmoError> {
        let path = self.root.join("plans").join(format!("{}.md", slug));

        match std::fs::read_to_string(&path) {
            Ok(content) => parser::parse_plan(&content, slug),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(OmoError::NotFound),
            Err(e) => Err(OmoError::Io(e)),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // -- helpers -----------------------------------------------------------

    /// Create a minimal plan file at `{dir}/plans/{slug}.md` with a title and
    /// optional checklist items.
    fn create_plan_file(dir: &std::path::Path, slug: &str, title: &str, items: &[&str]) {
        let plans_dir = dir.join("plans");
        std::fs::create_dir_all(&plans_dir).unwrap();
        let mut content = format!("# {} - Work Plan\n\n", title);
        for item in items {
            content.push_str(&format!("- [ ] {}\n", item));
        }
        let path = plans_dir.join(format!("{}.md", slug));
        std::fs::write(&path, content).unwrap();
    }

    // -- list_plans --------------------------------------------------------

    #[test]
    fn test_list_plans_returns_three() {
        let tmp = TempDir::new().unwrap();
        create_plan_file(tmp.path(), "plan-a", "Plan A", &["task a1", "task a2"]);
        create_plan_file(tmp.path(), "plan-b", "Plan B", &["task b1"]);
        create_plan_file(tmp.path(), "plan-c", "Plan C", &[]);

        let reader = FsPlanReader::new(tmp.path().to_path_buf());
        let plans = reader.list_plans().unwrap();
        assert_eq!(plans.len(), 3);

        // Every plan has the expected slug.
        let slugs: Vec<&str> = plans.iter().map(|p| p.slug.as_str()).collect();
        assert!(slugs.contains(&"plan-a"));
        assert!(slugs.contains(&"plan-b"));
        assert!(slugs.contains(&"plan-c"));
    }

    #[test]
    fn test_list_plans_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let plans_dir = tmp.path().join("plans");
        std::fs::create_dir_all(&plans_dir).unwrap();

        let reader = FsPlanReader::new(tmp.path().to_path_buf());
        let plans = reader.list_plans().unwrap();
        assert!(plans.is_empty());
    }

    #[test]
    fn test_list_plans_no_plans_dir() {
        // No `plans/` directory at all → empty vec, no error.
        let tmp = TempDir::new().unwrap();
        let reader = FsPlanReader::new(tmp.path().to_path_buf());
        let plans = reader.list_plans().unwrap();
        assert!(plans.is_empty());
    }

    #[test]
    fn test_list_plans_filters_non_md() {
        let tmp = TempDir::new().unwrap();
        let plans_dir = tmp.path().join("plans");
        std::fs::create_dir_all(&plans_dir).unwrap();

        // Valid .md file.
        std::fs::write(
            plans_dir.join("valid.md"),
            "# Valid - Work Plan\n- [ ] task\n",
        )
        .unwrap();
        // .txt file — should be skipped.
        std::fs::write(plans_dir.join("note.txt"), "not a plan").unwrap();
        // No extension — should be skipped.
        std::fs::write(plans_dir.join("README"), "# readme\n").unwrap();

        let reader = FsPlanReader::new(tmp.path().to_path_buf());
        let plans = reader.list_plans().unwrap();
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].slug, "valid");
    }

    #[test]
    fn test_list_plans_empty_md_file() {
        // A .md file with no content — parse_plan handles this gracefully.
        let tmp = TempDir::new().unwrap();
        let plans_dir = tmp.path().join("plans");
        std::fs::create_dir_all(&plans_dir).unwrap();
        std::fs::write(plans_dir.join("empty.md"), "").unwrap();

        let reader = FsPlanReader::new(tmp.path().to_path_buf());
        let plans = reader.list_plans().unwrap();
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].slug, "empty");
        // parse_plan falls back to slug as title when no H1 is found.
        assert_eq!(plans[0].title, "empty");
        assert!(plans[0].checklist.is_empty());
    }

    // -- read_plan ---------------------------------------------------------

    #[test]
    fn test_read_plan_success() {
        let tmp = TempDir::new().unwrap();
        create_plan_file(tmp.path(), "my-plan", "My Plan", &["step 1", "step 2"]);

        let reader = FsPlanReader::new(tmp.path().to_path_buf());
        let plan = reader.read_plan("my-plan").unwrap();
        assert_eq!(plan.slug, "my-plan");
        assert_eq!(plan.title, "My Plan");
        assert_eq!(plan.checklist.len(), 2);
    }

    #[test]
    fn test_read_plan_not_found() {
        let tmp = TempDir::new().unwrap();
        let reader = FsPlanReader::new(tmp.path().to_path_buf());
        let result = reader.read_plan("nonexistent");
        assert!(matches!(result, Err(OmoError::NotFound)));
    }

    // -- from_omo_home -----------------------------------------------------

    #[test]
    fn test_from_omo_home_ok() {
        let reader = FsPlanReader::from_omo_home().expect("from_omo_home should succeed");
        assert!(
            reader.root.to_string_lossy().ends_with(".omo"),
            "root should end with .omo, got: {}",
            reader.root.display()
        );
    }

    // -- discover_notepads (stub for Todo 4) -------------------------------

    #[test]
    fn test_discover_notepads_empty_when_no_notepads_dir() {
        let tmp = TempDir::new().unwrap();
        let reader = FsPlanReader::new(tmp.path().to_path_buf());
        let notepads = reader.discover_notepads();
        assert!(notepads.is_empty());
    }

    #[test]
    fn test_discover_notepads_skips_dirs_without_learnings() {
        let tmp = TempDir::new().unwrap();
        let notepads_dir = tmp.path().join("notepads");
        std::fs::create_dir_all(notepads_dir.join("some-project")).unwrap();

        let reader = FsPlanReader::new(tmp.path().to_path_buf());
        let notepads = reader.discover_notepads();
        assert!(notepads.is_empty());
    }
}
