use std::path::Path;

use crate::omo::types::OmoNotepad;

/// Default number of trailing lines to include in a notepad excerpt.
const DEFAULT_EXCERPT_LINES: usize = 10;

/// Discover notepad directories under `{root}/notepads/*/learnings.md`.
///
/// Reads file content via `std::fs::read()` with `String::from_utf8_lossy()` to
/// handle non-UTF-8 content gracefully. Returns the last 10 lines as the
/// excerpt. Returns an empty vec when the notepads directory is missing or
/// unreadable.
pub fn discover_notepads(root: &Path) -> Vec<OmoNotepad> {
    discover_notepads_with_lines(root, DEFAULT_EXCERPT_LINES)
}

/// Like `discover_notepads()` but with a configurable number of excerpt lines.
fn discover_notepads_with_lines(root: &Path, excerpt_lines: usize) -> Vec<OmoNotepad> {
    let notepads_dir = root.join("notepads");
    let mut notepads = Vec::new();

    let entries = match std::fs::read_dir(&notepads_dir) {
        Ok(e) => e,
        Err(_) => return notepads,
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let learnings_path = path.join("learnings.md");
        if !learnings_path.is_file() {
            continue;
        }

        let project = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };

        let excerpt = read_excerpt(&learnings_path, excerpt_lines);

        notepads.push(OmoNotepad {
            project,
            path: learnings_path,
            excerpt,
        });
    }

    notepads
}

/// Read the last `n` lines from a file, handling non-UTF-8 content gracefully.
fn read_excerpt(path: &Path, n: usize) -> String {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(_) => return String::new(),
    };

    let content = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = content.lines().collect();

    if lines.is_empty() {
        return "(empty)".to_string();
    }

    let start = if lines.len() > n { lines.len() - n } else { 0 };
    lines[start..].join("\n")
}

/// Map a plan slug to its matching notepad.
///
/// Extracts the "project" prefix from the slug by taking the first two
/// hyphen-separated segments (e.g. `fintesla-planishche-feature-x` →
/// `fintesla-planishche`) and searches for a notepad with that project name.
///
/// Returns `None` when no notepad matches — notepads are always optional.
pub fn plan_to_notepad<'a>(slug: &str, notepads: &'a [OmoNotepad]) -> Option<&'a OmoNotepad> {
    let prefix: String = slug.split('-').take(2).collect::<Vec<&str>>().join("-");

    if prefix.is_empty() {
        return None;
    }

    notepads.iter().find(|n| n.project == prefix)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Helper: create a learnings.md for a project under a temp root.
    fn create_learnings(root: &Path, project: &str, content: &str) {
        let project_dir = root.join("notepads").join(project);
        std::fs::create_dir_all(&project_dir).unwrap();
        std::fs::write(project_dir.join("learnings.md"), content).unwrap();
    }

    #[test]
    fn test_discover_notepads_returns_one() {
        let tmp = TempDir::new().unwrap();
        let lines_15: String = (1..=15)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        create_learnings(tmp.path(), "test-project", &lines_15);

        let notepads = discover_notepads(tmp.path());
        assert_eq!(notepads.len(), 1);
        assert_eq!(notepads[0].project, "test-project");

        // Excerpt should be last 10 lines (lines 6–15).
        let expected: String = (6..=15)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(notepads[0].excerpt, expected);
    }

    #[test]
    fn test_discover_notepads_empty_file() {
        let tmp = TempDir::new().unwrap();
        create_learnings(tmp.path(), "empty-project", "");

        let notepads = discover_notepads(tmp.path());
        assert_eq!(notepads.len(), 1);
        assert_eq!(notepads[0].excerpt, "(empty)");
    }

    #[test]
    fn test_discover_notepads_missing_dir() {
        let tmp = TempDir::new().unwrap();
        let notepads = discover_notepads(tmp.path());
        assert!(notepads.is_empty());
    }

    #[test]
    fn test_discover_notepads_skips_dirs_without_learnings() {
        let tmp = TempDir::new().unwrap();
        let notepads_dir = tmp.path().join("notepads");
        std::fs::create_dir_all(notepads_dir.join("some-project")).unwrap();

        let notepads = discover_notepads(tmp.path());
        assert!(notepads.is_empty());
    }

    #[test]
    fn test_discover_notepads_non_utf8() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("notepads").join("binary-proj");
        std::fs::create_dir_all(&project_dir).unwrap();
        let path = project_dir.join("learnings.md");
        // 0xFF is invalid UTF-8 — from_utf8_lossy replaces with U+FFFD.
        std::fs::write(&path, b"\xff\xfe\xfd\xfc line content").unwrap();

        let notepads = discover_notepads(tmp.path());
        assert_eq!(notepads.len(), 1);
        assert!(notepads[0].excerpt.contains('\u{fffd}'));
        assert!(notepads[0].excerpt.contains("line content"));
    }

    #[test]
    fn test_plan_to_notepad_finds_match() {
        let tmp = TempDir::new().unwrap();
        create_learnings(tmp.path(), "fintesla-planishche", "some content");

        let notepads = discover_notepads(tmp.path());
        let result = plan_to_notepad("fintesla-planishche-feature-x", &notepads);
        assert!(result.is_some());
        assert_eq!(result.unwrap().project, "fintesla-planishche");
    }

    #[test]
    fn test_plan_to_notepad_no_match() {
        let tmp = TempDir::new().unwrap();
        create_learnings(tmp.path(), "other-project", "content");

        let notepads = discover_notepads(tmp.path());
        let result = plan_to_notepad("unrelated-plan", &notepads);
        assert!(result.is_none());
    }

    #[test]
    fn test_plan_to_notepad_empty_slug() {
        let tmp = TempDir::new().unwrap();
        create_learnings(tmp.path(), "some-project", "content");

        let notepads = discover_notepads(tmp.path());
        let result = plan_to_notepad("", &notepads);
        assert!(result.is_none());
    }

    #[test]
    fn test_discover_notepads_excerpt_under_ten_lines() {
        // File with fewer than 10 lines → whole file becomes excerpt.
        let tmp = TempDir::new().unwrap();
        let lines_3 = "alpha\nbeta\ngamma".to_string();
        create_learnings(tmp.path(), "small-proj", &lines_3);

        let notepads = discover_notepads(tmp.path());
        assert_eq!(notepads.len(), 1);
        assert_eq!(notepads[0].excerpt, "alpha\nbeta\ngamma");
    }

    #[test]
    fn test_discover_notepads_excerpt_only_newlines() {
        // A file with only newlines — lines() yields empty strings, not "(empty)".
        let tmp = TempDir::new().unwrap();
        create_learnings(tmp.path(), "newline-only", "\n\n\n");

        let notepads = discover_notepads(tmp.path());
        assert_eq!(notepads.len(), 1);
        // Three empty strings joined by \n = two newlines.
        assert_eq!(notepads[0].excerpt, "\n\n");
    }
}
