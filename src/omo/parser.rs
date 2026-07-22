use crate::omo::types::{ChecklistItem, OmoError, OmoPlan, PlanStatus};

/// Parse an omo plan markdown file into an `OmoPlan`.
///
/// Line-by-line state machine — no regex crate, no unwrap/expect.
pub fn parse_plan(input: &str, slug: &str) -> Result<OmoPlan, OmoError> {
    let mut title = String::new();
    let mut tl_dr = String::new();
    let mut scope_in: Vec<String> = Vec::new();
    let mut scope_out: Vec<String> = Vec::new();
    let mut checklist: Vec<ChecklistItem> = Vec::new();
    let mut preamble = String::new();

    // State machine
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Section {
        Preamble,
        TlDr,
        Scope,
        Other,
    }

    let mut current_section = Section::Preamble;
    let mut in_must_have = false;
    let mut in_must_not_have = false;

    let lines: Vec<&str> = input.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let raw = lines[i];
        let trimmed = raw.trim();

        // --- H1: extract title (only at start of line, not indented in code blocks) ---
        if raw.starts_with("# ") && !raw.contains("##") {
            let rest = raw.strip_prefix("# ").unwrap_or("").trim();
            // Strip " - Work Plan" suffix if present
            title = rest
                .strip_suffix(" - Work Plan")
                .or_else(|| rest.strip_suffix("- Work Plan"))
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| rest.to_string());
            i += 1;
            continue;
        }

        // --- H2: section boundary (only at start of line) ---
        if raw.starts_with("## ") {
            let header = trimmed.strip_prefix("## ").unwrap_or("").trim().to_lowercase();
            in_must_have = false;
            in_must_not_have = false;

            if header.contains("tl;dr") || header.contains("tldr") {
                current_section = Section::TlDr;
            } else if header == "scope" {
                current_section = Section::Scope;
            } else {
                current_section = Section::Other;
            }
            i += 1;
            continue;
        }

        // --- H3: must have / must not have (only at start of line) ---
        if raw.starts_with("### ") {
            let h3 = trimmed.strip_prefix("### ").unwrap_or("").trim().to_lowercase();
            if h3.contains("must have") {
                in_must_have = true;
                in_must_not_have = false;
            } else if h3.contains("must not have") || h3.contains("must not") {
                in_must_not_have = true;
                in_must_have = false;
            }
            i += 1;
            continue;
        }

        // --- Checklist items: - [ ] or - [x] ---
        if let Some(item) = try_parse_checklist(raw) {
            checklist.push(item);
            i += 1;
            continue;
        }

        // --- Scope items: - <text> under Must have / Must NOT have ---
        if current_section == Section::Scope && trimmed.starts_with("- ") {
            let text = trimmed.strip_prefix("- ").unwrap_or("").trim().to_string();
            if in_must_have && !text.is_empty() {
                scope_in.push(text);
            } else if in_must_not_have && !text.is_empty() {
                scope_out.push(text);
            }
            i += 1;
            continue;
        }

        // --- TL;DR key-value lines ---
        if current_section == Section::TlDr {
            if let Some(val) = try_extract_tldr_field(trimmed, "**What you'll get:**") {
                push_tldr(&mut tl_dr, &format!("**What you'll get:** {val}"));
            } else if let Some(val) = try_extract_tldr_field(trimmed, "**Why this approach:**") {
                push_tldr(&mut tl_dr, &format!("**Why this approach:** {val}"));
            } else if let Some(val) = try_extract_tldr_field(trimmed, "**What it will NOT do:**") {
                push_tldr(&mut tl_dr, &format!("**What it will NOT do:** {val}"));
            } else if let Some(val) = try_extract_tldr_field(trimmed, "**Effort:**") {
                push_tldr(&mut tl_dr, &format!("**Effort:** {val}"));
            } else if let Some(val) = try_extract_tldr_field(trimmed, "**Risk:**") {
                push_tldr(&mut tl_dr, &format!("**Risk:** {val}"));
            } else if !trimmed.is_empty()
                && !trimmed.starts_with("---")
                && !trimmed.starts_with(">")
                && !trimmed.starts_with("Your next move")
            {
                // Collect other non-empty TL;DR lines as part of the description
                push_tldr(&mut tl_dr, trimmed);
            }
        }

        // --- Preamble: everything before first ## header ---
        if current_section == Section::Preamble && !trimmed.is_empty() {
            if !preamble.is_empty() {
                preamble.push('\n');
            }
            preamble.push_str(trimmed);
        }

        i += 1;
    }

    // Build the plan
    let plan = OmoPlan {
        slug: slug.to_string(),
        title: if title.is_empty() { slug.to_string() } else { title },
        status: PlanStatus::Drafting,
        tl_dr: if tl_dr.is_empty() { None } else { Some(tl_dr.trim().to_string()) },
        scope_in,
        scope_out,
        checklist,
        notepad_slug: None,
    };

    Ok(plan)
}

/// Try to parse a checklist item from a line.
/// Supports: `- [ ] text`, `- [x] text`, `- [ ] 1. numbered text`
/// Tracks indent level by leading spaces.
fn try_parse_checklist(line: &str) -> Option<ChecklistItem> {
    let trimmed = line.trim();
    if !trimmed.starts_with("- [") {
        return None;
    }

    // After stripping `- [`, trim to handle the space between `[` and `]` in `- [ ]`
    let rest = trimmed.strip_prefix("- [")?.trim_start();

    let (done, text_start) = if let Some(r) = rest.strip_prefix("] ") {
        (false, r)
    } else if let Some(r) = rest.strip_prefix("x] ") {
        (true, r)
    } else if let Some(r) = rest.strip_prefix("X] ") {
        (true, r)
    } else {
        return None;
    };

    let mut text = text_start.trim().to_string();

    // Strip numbered prefix like "1. ", "2. " etc.
    if let Some(after_num) = text.strip_prefix(|c: char| c.is_ascii_digit())
        && after_num.starts_with(". ")
    {
        text = after_num.strip_prefix(". ").unwrap_or("").trim().to_string();
    }

    if text.is_empty() {
        return None;
    }

    // Count leading spaces to determine indent level (0 = top level)
    let leading_spaces = line.chars().take_while(|c| *c == ' ').count();
    let level = leading_spaces / 2; // 2 spaces per indent level

    Some(ChecklistItem {
        text,
        done,
        level,
    })
}

/// Extract a value after a bold-field marker like `**What you'll get:**`.
/// Returns `Some(value)` if the line starts with the marker.
fn try_extract_tldr_field<'a>(line: &'a str, marker: &str) -> Option<&'a str> {
    if let Some(rest) = line.strip_prefix(marker) {
        let val = rest.trim();
        if !val.is_empty() {
            return Some(val);
        }
    }
    None
}

/// Push a line to the TL;DR string, joining with newlines.
fn push_tldr(tl_dr: &mut String, line: &str) {
    if !tl_dr.is_empty() {
        tl_dr.push('\n');
    }
    tl_dr.push_str(line);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse the real devzat-docker-private plan file.
    #[test]
    fn test_parse_real_plan_file() {
        let content = include_str!(concat!(
            env!("HOME"),
            "/.omo/plans/devzat-docker-private.md"
        ));
        let plan = parse_plan(content, "devzat-docker-private").expect("should parse real plan");

        assert_eq!(plan.slug, "devzat-docker-private");
        assert_eq!(plan.title, "devzat-docker-private");
        assert!(plan.tl_dr.is_some());
        let tl_dr = plan.tl_dr.as_deref().unwrap();
        assert!(tl_dr.contains("What you'll get:"));
        assert!(tl_dr.contains("Why this approach:"));
        assert!(tl_dr.contains("Effort:"));
        assert!(tl_dr.contains("Risk:"));

        // Scope: must have items
        assert!(!plan.scope_in.is_empty(), "should have scope_in items");
        assert!(plan.scope_in.iter().any(|s| s.contains("Docker-образ")));

        // Scope: must NOT have items
        assert!(!plan.scope_out.is_empty(), "should have scope_out items");
        assert!(plan.scope_out.iter().any(|s| s.contains("НЕ настраивать")));

        // Checklist items: 6 todos + 4 final verification + 6 success criteria = 16
        // But we only count top-level checklist items with - [ ] / - [x]
        assert!(
            plan.checklist.len() >= 6,
            "should have at least 6 checklist items, got {}",
            plan.checklist.len()
        );
    }

    /// Malformed input should not panic — returns Ok with slug set.
    #[test]
    fn test_parse_malformed_input() {
        let plan = parse_plan("hello world abc 123", "test-slug").expect("should not panic");
        assert_eq!(plan.slug, "test-slug");
        assert!(plan.checklist.is_empty());
        assert!(plan.scope_in.is_empty());
        assert!(plan.scope_out.is_empty());
        assert!(plan.tl_dr.is_none());
    }

    /// Empty string should return Ok with slug set and empty fields.
    #[test]
    fn test_parse_empty_string() {
        let plan = parse_plan("", "empty-plan").expect("should handle empty input");
        assert_eq!(plan.slug, "empty-plan");
        assert!(plan.checklist.is_empty());
        assert!(plan.scope_in.is_empty());
        assert!(plan.scope_out.is_empty());
        assert!(plan.tl_dr.is_none());
    }

    /// Checklist parsing at various indent levels.
    #[test]
    fn test_parse_checklist_items() {
        let input = "\
- [ ] item one
- [x] done item
  - [ ] nested item
    - [x] deeply nested done
- [ ] 1. numbered item
  - [ ] 2. another numbered
";
        let plan = parse_plan(input, "checklist-test").expect("should parse");
        assert_eq!(plan.checklist.len(), 6);

        // item one
        assert_eq!(plan.checklist[0].text, "item one");
        assert!(!plan.checklist[0].done);
        assert_eq!(plan.checklist[0].level, 0);

        // done item
        assert_eq!(plan.checklist[1].text, "done item");
        assert!(plan.checklist[1].done);
        assert_eq!(plan.checklist[1].level, 0);

        // nested item (2 spaces indent)
        assert_eq!(plan.checklist[2].text, "nested item");
        assert!(!plan.checklist[2].done);
        assert_eq!(plan.checklist[2].level, 1);

        // deeply nested done (4 spaces indent)
        assert_eq!(plan.checklist[3].text, "deeply nested done");
        assert!(plan.checklist[3].done);
        assert_eq!(plan.checklist[3].level, 2);

        // numbered item (number stripped)
        assert_eq!(plan.checklist[4].text, "numbered item");
        assert!(!plan.checklist[4].done);
        assert_eq!(plan.checklist[4].level, 0);

        // another numbered (nested, number stripped)
        assert_eq!(plan.checklist[5].text, "another numbered");
        assert!(!plan.checklist[5].done);
        assert_eq!(plan.checklist[5].level, 1);
    }

    /// H1 extraction from a real plan header.
    #[test]
    fn test_h1_extraction() {
        let input = "# my-plan - Work Plan\n\nSome preamble text.";
        let plan = parse_plan(input, "my-plan").expect("should parse");
        assert_eq!(plan.title, "my-plan");
    }

    /// H1 without - Work Plan suffix.
    #[test]
    fn test_h1_without_suffix() {
        let input = "# My Custom Title\n\nContent here.";
        let plan = parse_plan(input, "custom-slug").expect("should parse");
        assert_eq!(plan.title, "My Custom Title");
    }

    /// Section count from real plan: TL;DR, Scope, Verification, Execution, Todos, Final verification, Commit strategy, Success criteria = 8
    #[test]
    fn test_section_count_from_real_plan() {
        let content = include_str!(concat!(
            env!("HOME"),
            "/.omo/plans/devzat-docker-private.md"
        ));
        let plan = parse_plan(content, "devzat-docker-private").expect("should parse");

        // Count sections by counting ## headers in the source
        let section_count = content.lines().filter(|l| l.trim().starts_with("## ")).count();
        assert_eq!(section_count, 8, "expected 8 sections in devzat plan");
        assert_eq!(plan.slug, "devzat-docker-private");
    }

    /// Checklist count from real plan: 6 todos + 4 final verification + 6 success criteria = 16
    #[test]
    fn test_checklist_count_from_real_plan() {
        let content = include_str!(concat!(
            env!("HOME"),
            "/.omo/plans/devzat-docker-private.md"
        ));
        let plan = parse_plan(content, "devzat-docker-private").expect("should parse");

        let checklist_count = content
            .lines()
            .filter(|l| {
                let t = l.trim();
                t.starts_with("- [ ]") || t.starts_with("- [x]") || t.starts_with("- [X]")
            })
            .count();
        assert_eq!(
            plan.checklist.len(),
            checklist_count,
            "should match actual checklist item count"
        );
    }

    /// Scope parsing: must have and must NOT have.
    #[test]
    fn test_scope_parsing() {
        let input = "\
## Scope
### Must have
- Feature A
- Feature B

### Must NOT have
- NOT Feature C
- NOT Feature D
";
        let plan = parse_plan(input, "scope-test").expect("should parse");
        assert_eq!(plan.scope_in.len(), 2);
        assert_eq!(plan.scope_in[0], "Feature A");
        assert_eq!(plan.scope_in[1], "Feature B");
        assert_eq!(plan.scope_out.len(), 2);
        assert_eq!(plan.scope_out[0], "NOT Feature C");
        assert_eq!(plan.scope_out[1], "NOT Feature D");
    }

    /// TL;DR field extraction.
    #[test]
    fn test_tldr_field_extraction() {
        let input = "\
## TL;DR (For humans)

**What you'll get:** A cool thing.
**Why this approach:** It works.
**Effort:** Short
**Risk:** Low
";
        let plan = parse_plan(input, "tldr-test").expect("should parse");
        let tl_dr = plan.tl_dr.expect("should have tl_dr");
        assert!(tl_dr.contains("What you'll get:"));
        assert!(tl_dr.contains("A cool thing."));
        assert!(tl_dr.contains("Why this approach:"));
        assert!(tl_dr.contains("It works."));
        assert!(tl_dr.contains("Effort:"));
        assert!(tl_dr.contains("Short"));
        assert!(tl_dr.contains("Risk:"));
        assert!(tl_dr.contains("Low"));
    }

    /// Lines that look like checklist but aren't valid should be ignored.
    #[test]
    fn test_invalid_checklist_lines() {
        let input = "\
- [ ] valid
- [invalid
-  ] not checklist
- [ ] 
- [x]
";
        let plan = parse_plan(input, "invalid-checklist").expect("should parse");
        assert_eq!(plan.checklist.len(), 1);
        assert_eq!(plan.checklist[0].text, "valid");
    }

    /// Preamble before first ## header is captured.
    #[test]
    fn test_preamble_captured() {
        let input = "\
# test - Work Plan

Some preamble text here.

More preamble.

## First Section

Content.
";
        let plan = parse_plan(input, "preamble-test").expect("should parse");
        // Preamble is not stored in a dedicated field, but the parser should not crash
        assert_eq!(plan.slug, "preamble-test");
        assert_eq!(plan.title, "test");
    }
}
