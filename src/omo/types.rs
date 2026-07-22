use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::omo::reader::PlanReader;

// ---------------------------------------------------------------------------
// OmoError
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum OmoError {
    Io(std::io::Error),
    Parse(String),
    NotFound,
    NoOmoHome,
}

impl PartialEq for OmoError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (OmoError::Io(a), OmoError::Io(b)) => a.kind() == b.kind(),
            (OmoError::Parse(a), OmoError::Parse(b)) => a == b,
            (OmoError::NotFound, OmoError::NotFound) => true,
            (OmoError::NoOmoHome, OmoError::NoOmoHome) => true,
            _ => false,
        }
    }
}

impl fmt::Display for OmoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OmoError::Io(e) => write!(f, "I/O error: {e}"),
            OmoError::Parse(msg) => write!(f, "parse error: {msg}"),
            OmoError::NotFound => write!(f, "plan not found"),
            OmoError::NoOmoHome => write!(f, "OMO_HOME not set"),
        }
    }
}

impl std::error::Error for OmoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            OmoError::Io(e) => Some(e),
            OmoError::Parse(_) | OmoError::NotFound | OmoError::NoOmoHome => None,
        }
    }
}

impl From<std::io::Error> for OmoError {
    fn from(e: std::io::Error) -> Self {
        OmoError::Io(e)
    }
}

// ---------------------------------------------------------------------------
// PlanStatus
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanStatus {
    Drafting,
    Active,
    Completed,
}

impl fmt::Display for PlanStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlanStatus::Drafting => write!(f, "drafting"),
            PlanStatus::Active => write!(f, "active"),
            PlanStatus::Completed => write!(f, "completed"),
        }
    }
}

impl FromStr for PlanStatus {
    type Err = OmoError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "drafting" => Ok(PlanStatus::Drafting),
            "active" => Ok(PlanStatus::Active),
            "completed" => Ok(PlanStatus::Completed),
            _ => Err(OmoError::Parse(format!("invalid plan status: {s}"))),
        }
    }
}

// ---------------------------------------------------------------------------
// ChecklistItem
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistItem {
    pub text: String,
    pub done: bool,
    pub level: usize,
}

// ---------------------------------------------------------------------------
// OmoPlan
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OmoPlan {
    pub slug: String,
    pub title: String,
    pub status: PlanStatus,
    pub tl_dr: Option<String>,
    pub scope_in: Vec<String>,
    pub scope_out: Vec<String>,
    pub checklist: Vec<ChecklistItem>,
    pub notepad_slug: Option<String>,
}

// ---------------------------------------------------------------------------
// PlanCard
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanCard {
    pub slug: String,
    pub title: String,
    pub status: PlanStatus,
    pub checklist_total: usize,
    pub checklist_done: usize,
}

// ---------------------------------------------------------------------------
// OmoNotepad
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OmoNotepad {
    pub project: String,
    pub path: PathBuf,
    pub excerpt: String,
}

// ---------------------------------------------------------------------------
// OmoState
// ---------------------------------------------------------------------------

/// Runtime state for omo plan management within the kanban app.
/// Holds a type-erased `PlanReader`, the currently selected plan slug,
/// and discovered notepads for excerpt enrichment in the detail overlay.
pub struct OmoState {
    pub reader: Box<dyn PlanReader>,
    pub active_plan_slug: Option<String>,
    pub plans_loaded: bool,
    pub notepads: Vec<OmoNotepad>,
}

impl std::fmt::Debug for OmoState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OmoState")
            .field("active_plan_slug", &self.active_plan_slug)
            .field("plans_loaded", &self.plans_loaded)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_status_display() {
        assert_eq!(PlanStatus::Drafting.to_string(), "drafting");
        assert_eq!(PlanStatus::Active.to_string(), "active");
        assert_eq!(PlanStatus::Completed.to_string(), "completed");
    }

    #[test]
    fn test_plan_status_from_str() {
        assert_eq!("drafting".parse::<PlanStatus>(), Ok(PlanStatus::Drafting));
        assert_eq!("active".parse::<PlanStatus>(), Ok(PlanStatus::Active));
        assert_eq!("completed".parse::<PlanStatus>(), Ok(PlanStatus::Completed));
    }

    #[test]
    fn test_plan_status_from_str_case_insensitive() {
        assert_eq!("Drafting".parse::<PlanStatus>(), Ok(PlanStatus::Drafting));
        assert_eq!("ACTIVE".parse::<PlanStatus>(), Ok(PlanStatus::Active));
        assert_eq!("Completed".parse::<PlanStatus>(), Ok(PlanStatus::Completed));
    }

    #[test]
    fn test_plan_status_from_str_invalid() {
        let result: Result<PlanStatus, OmoError> = "invalid".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_omo_plan_construction() {
        let plan = OmoPlan {
            slug: "test-plan".into(),
            title: "Test Plan".into(),
            status: PlanStatus::Active,
            tl_dr: Some("A test plan".into()),
            scope_in: vec!["feature-a".into()],
            scope_out: vec!["feature-b".into()],
            checklist: vec![
                ChecklistItem {
                    text: "Step 1".into(),
                    done: true,
                    level: 0,
                },
                ChecklistItem {
                    text: "Step 2".into(),
                    done: false,
                    level: 1,
                },
            ],
            notepad_slug: Some("notes".into()),
        };

        assert_eq!(plan.slug, "test-plan");
        assert_eq!(plan.title, "Test Plan");
        assert_eq!(plan.status, PlanStatus::Active);
        assert_eq!(plan.tl_dr.as_deref(), Some("A test plan"));
        assert_eq!(plan.scope_in, vec!["feature-a"]);
        assert_eq!(plan.scope_out, vec!["feature-b"]);
        assert_eq!(plan.checklist.len(), 2);
        assert!(plan.checklist[0].done);
        assert!(!plan.checklist[1].done);
        assert_eq!(plan.notepad_slug.as_deref(), Some("notes"));
    }

    #[test]
    fn test_plan_card_from_plan() {
        let plan = OmoPlan {
            slug: "card-test".into(),
            title: "Card Test".into(),
            status: PlanStatus::Drafting,
            tl_dr: None,
            scope_in: vec![],
            scope_out: vec![],
            checklist: vec![
                ChecklistItem {
                    text: "A".into(),
                    done: true,
                    level: 0,
                },
                ChecklistItem {
                    text: "B".into(),
                    done: false,
                    level: 0,
                },
                ChecklistItem {
                    text: "C".into(),
                    done: true,
                    level: 1,
                },
            ],
            notepad_slug: None,
        };

        let card = PlanCard {
            slug: plan.slug.clone(),
            title: plan.title.clone(),
            status: plan.status,
            checklist_total: plan.checklist.len(),
            checklist_done: plan.checklist.iter().filter(|c| c.done).count(),
        };

        assert_eq!(card.slug, "card-test");
        assert_eq!(card.title, "Card Test");
        assert_eq!(card.status, PlanStatus::Drafting);
        assert_eq!(card.checklist_total, 3);
        assert_eq!(card.checklist_done, 2);
    }

    #[test]
    fn test_omo_error_display() {
        let io_err = OmoError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "file missing"));
        assert!(io_err.to_string().contains("I/O error"));

        let parse_err = OmoError::Parse("bad format".into());
        assert_eq!(parse_err.to_string(), "parse error: bad format");

        let nf = OmoError::NotFound;
        assert_eq!(nf.to_string(), "plan not found");

        let no_home = OmoError::NoOmoHome;
        assert_eq!(no_home.to_string(), "OMO_HOME not set");
    }
}
