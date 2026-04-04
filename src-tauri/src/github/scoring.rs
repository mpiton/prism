use crate::types::{CiStatus, Priority, PullRequest};

/// Maximum label weight found in the PR's labels.
///
/// Weights: `critical` = 10, `bug` = 7, `enhancement` = 3, `docs` = 1,
/// known deprioritisation labels = 0, any other label = 2, no labels = 0.
fn label_weight(labels: &[String]) -> f64 {
    labels
        .iter()
        .map(|l| match l.to_lowercase().as_str() {
            "critical" => 10.0,
            "bug" => 7.0,
            "enhancement" => 3.0,
            "docs" | "documentation" => 1.0,
            "wontfix" | "invalid" | "duplicate" | "on-hold" => 0.0,
            _ => 2.0,
        })
        .fold(0.0_f64, f64::max)
}

/// Compute the age of a PR in hours from `created_at` (ISO 8601) to `now`.
///
/// Returns 0.0 if the timestamp cannot be parsed.
fn age_hours(created_at: &str, now: &str) -> f64 {
    let parse = |s: &str| {
        chrono::DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|dt| dt.with_timezone(&chrono::Utc))
    };

    match (parse(created_at), parse(now)) {
        (Some(created), Some(now_dt)) => {
            let duration = now_dt.signed_duration_since(created);
            #[allow(clippy::cast_precision_loss)]
            (duration.num_minutes() as f64 / 60.0).max(0.0)
        }
        _ => 0.0,
    }
}

/// Compute a priority score for a pull request.
///
/// Formula: `score = (label_weight × 3) + (age_hours × 0.5) + (small_diff ? 2 : 0) + (ci_pass ? 1 : 0)`
///
/// - `label_weight`: max weight across labels (critical=10, bug=7, enhancement=3, docs=1, default=2)
/// - `age_hours`: hours since `created_at` (capped at 48h to prevent unbounded escalation)
/// - `small_diff`: `additions + deletions < 200`
/// - `ci_pass`: `ci_status == Success`
#[allow(dead_code)]
pub fn compute_priority_score(pr: &PullRequest) -> f64 {
    compute_priority_score_at(pr, &chrono::Utc::now().to_rfc3339())
}

/// Testable version that accepts an explicit "now" timestamp.
pub(crate) fn compute_priority_score_at(pr: &PullRequest, now: &str) -> f64 {
    let lw = label_weight(&pr.labels);
    let age = age_hours(&pr.created_at, now).min(48.0);
    let small_diff = u64::from(pr.additions) + u64::from(pr.deletions) < 200;
    let ci_pass = pr.ci_status == CiStatus::Success;

    (lw * 3.0)
        + (age * 0.5)
        + (if small_diff { 2.0 } else { 0.0 })
        + (if ci_pass { 1.0 } else { 0.0 })
}

/// Derive a `Priority` from a numeric score.
///
/// Thresholds:
/// - `>= 25` → Critical
/// - `>= 15` → High
/// - `>= 8`  → Medium
/// - `< 8`   → Low
#[allow(dead_code)]
pub fn compute_priority(score: f64) -> Priority {
    if score >= 25.0 {
        Priority::Critical
    } else if score >= 15.0 {
        Priority::High
    } else if score >= 8.0 {
        Priority::Medium
    } else {
        Priority::Low
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CiStatus, PrState, Priority};

    const NOW: &str = "2026-03-26T10:00:00Z";

    fn make_pr(overrides: PrOverrides) -> PullRequest {
        PullRequest {
            id: "pr-1".to_string(),
            number: 42,
            title: "Test PR".to_string(),
            author: "alice".to_string(),
            state: PrState::Open,
            ci_status: overrides.ci_status.unwrap_or(CiStatus::Pending),
            priority: Priority::Medium,
            repo_id: "repo-1".to_string(),
            url: "https://github.com/org/repo/pull/42".to_string(),
            labels: overrides.labels.unwrap_or_default(),
            additions: overrides.additions.unwrap_or(50),
            deletions: overrides.deletions.unwrap_or(10),
            head_ref_name: "fix/test-branch".to_string(),
            created_at: overrides
                .created_at
                .unwrap_or_else(|| "2026-03-25T10:00:00Z".to_string()),
            updated_at: "2026-03-26T10:00:00Z".to_string(),
        }
    }

    #[derive(Default)]
    struct PrOverrides {
        labels: Option<Vec<String>>,
        ci_status: Option<CiStatus>,
        additions: Option<u32>,
        deletions: Option<u32>,
        created_at: Option<String>,
    }

    #[test]
    fn test_score_critical_label() {
        let pr = make_pr(PrOverrides {
            labels: Some(vec!["critical".to_string()]),
            ..Default::default()
        });
        let score = compute_priority_score_at(&pr, NOW);
        // label_weight=10 → 10*3=30, plus age and diff bonuses
        assert!(
            score >= 30.0,
            "critical label should yield score >= 30, got {score}"
        );
    }

    #[test]
    fn test_score_small_diff_bonus() {
        let small = make_pr(PrOverrides {
            additions: Some(50),
            deletions: Some(10),
            ..Default::default()
        });
        let large = make_pr(PrOverrides {
            additions: Some(150),
            deletions: Some(100),
            ..Default::default()
        });
        let score_small = compute_priority_score_at(&small, NOW);
        let score_large = compute_priority_score_at(&large, NOW);
        assert!(
            score_small > score_large,
            "small diff ({score_small}) should score higher than large diff ({score_large})"
        );
    }

    #[test]
    fn test_score_ci_pass_bonus() {
        let passing = make_pr(PrOverrides {
            ci_status: Some(CiStatus::Success),
            ..Default::default()
        });
        let failing = make_pr(PrOverrides {
            ci_status: Some(CiStatus::Failure),
            ..Default::default()
        });
        let score_pass = compute_priority_score_at(&passing, NOW);
        let score_fail = compute_priority_score_at(&failing, NOW);
        assert!(
            score_pass > score_fail,
            "CI pass ({score_pass}) should score higher than CI fail ({score_fail})"
        );
    }

    #[test]
    fn test_score_age_factor() {
        let old = make_pr(PrOverrides {
            created_at: Some("2026-03-01T10:00:00Z".to_string()),
            ..Default::default()
        });
        let recent = make_pr(PrOverrides {
            created_at: Some("2026-03-26T09:00:00Z".to_string()),
            ..Default::default()
        });
        let score_old = compute_priority_score_at(&old, NOW);
        let score_recent = compute_priority_score_at(&recent, NOW);
        assert!(
            score_old > score_recent,
            "older PR ({score_old}) should score higher than recent PR ({score_recent})"
        );
    }

    #[test]
    fn test_priority_from_score_critical() {
        assert_eq!(compute_priority(30.0), Priority::Critical);
        assert_eq!(compute_priority(25.0), Priority::Critical);
    }

    #[test]
    fn test_priority_from_score_low() {
        assert_eq!(compute_priority(0.0), Priority::Low);
        assert_eq!(compute_priority(7.9), Priority::Low);
    }

    #[test]
    fn test_score_no_labels() {
        let pr = make_pr(PrOverrides {
            labels: Some(vec![]),
            ..Default::default()
        });
        let score = compute_priority_score_at(&pr, NOW);
        // No labels → label_weight=0 → 0*3=0, only age + diff + ci bonuses
        assert!(
            score < 25.0,
            "no-label PR should not reach critical threshold, got {score}"
        );
    }

    #[test]
    fn test_priority_thresholds() {
        assert_eq!(compute_priority(25.0), Priority::Critical);
        assert_eq!(compute_priority(24.9), Priority::High);
        assert_eq!(compute_priority(15.0), Priority::High);
        assert_eq!(compute_priority(14.9), Priority::Medium);
        assert_eq!(compute_priority(8.0), Priority::Medium);
        assert_eq!(compute_priority(7.9), Priority::Low);
    }

    #[test]
    fn test_small_diff_threshold_boundary() {
        let at_199 = make_pr(PrOverrides {
            additions: Some(199),
            deletions: Some(0),
            ..Default::default()
        });
        let at_200 = make_pr(PrOverrides {
            additions: Some(200),
            deletions: Some(0),
            ..Default::default()
        });
        let s199 = compute_priority_score_at(&at_199, NOW);
        let s200 = compute_priority_score_at(&at_200, NOW);
        assert!(
            s199 > s200,
            "199 should receive small-diff bonus; 200 should not"
        );
    }

    #[test]
    fn test_age_capped_at_48h() {
        let very_old = make_pr(PrOverrides {
            created_at: Some("2025-01-01T00:00:00Z".to_string()),
            ..Default::default()
        });
        let at_48h = make_pr(PrOverrides {
            created_at: Some("2026-03-24T10:00:00Z".to_string()),
            ..Default::default()
        });
        let score_very_old = compute_priority_score_at(&very_old, NOW);
        let score_48h = compute_priority_score_at(&at_48h, NOW);
        assert!(
            (score_very_old - score_48h).abs() < 0.01,
            "age should be capped at 48h: very_old={score_very_old}, 48h={score_48h}"
        );
    }

    #[test]
    fn test_deprioritisation_labels_zero_weight() {
        for label in &["wontfix", "invalid", "duplicate", "on-hold"] {
            let pr = make_pr(PrOverrides {
                labels: Some(vec![label.to_string()]),
                ..Default::default()
            });
            let no_label = make_pr(PrOverrides {
                labels: Some(vec![]),
                ..Default::default()
            });
            let score_deprio = compute_priority_score_at(&pr, NOW);
            let score_none = compute_priority_score_at(&no_label, NOW);
            assert!(
                (score_deprio - score_none).abs() < 0.01,
                "label '{label}' should have zero weight like no labels"
            );
        }
    }

    #[test]
    fn test_label_weight_uses_max() {
        let pr = make_pr(PrOverrides {
            labels: Some(vec![
                "docs".to_string(),
                "bug".to_string(),
                "enhancement".to_string(),
            ]),
            ..Default::default()
        });
        let score = compute_priority_score_at(&pr, NOW);
        // Max label weight = bug=7 → 7*3=21, check it's at least 21
        assert!(
            score >= 21.0,
            "bug label (max) should contribute at least 21, got {score}"
        );
    }
}
