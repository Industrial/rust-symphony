//! Label-based candidate filtering (SPEC_ADDENDUM_1 A.1).
//!
//! Apply include_labels (whitelist) then exclude_labels (blacklist).
//! Comparison uses lowercase normalization to match GitHub behaviour.

use symphony_domain::Issue;

/// Apply include_labels then exclude_labels to a list of issues.
/// Issue labels are assumed already lowercase (from GitHub normalization).
/// Config labels are normalized to lowercase for comparison.
///
/// - include_labels: if Some and non-empty, retain only issues that have at least one of these labels.
/// - exclude_labels: if Some and non-empty, drop issues that have any of these labels.
pub fn apply_label_filters(
  issues: Vec<Issue>,
  include_labels: Option<&[String]>,
  exclude_labels: Option<&[String]>,
) -> Vec<Issue> {
  let mut out = issues;

  if let Some(include) = include_labels {
    if !include.is_empty() {
      let include_lower: std::collections::HashSet<String> =
        include.iter().map(|s| s.to_lowercase()).collect();
      out.retain(|issue| {
        issue
          .labels
          .iter()
          .any(|l| include_lower.contains(&l.to_lowercase()))
      });
    }
  }

  if let Some(exclude) = exclude_labels {
    if !exclude.is_empty() {
      let exclude_lower: std::collections::HashSet<String> =
        exclude.iter().map(|s| s.to_lowercase()).collect();
      out.retain(|issue| {
        !issue
          .labels
          .iter()
          .any(|l| exclude_lower.contains(&l.to_lowercase()))
      });
    }
  }

  out
}

/// Return true if a single issue would pass the include/exclude label filters (SPEC_ADDENDUM_1 A.4 retry).
pub fn issue_passes_label_filters(
  issue: &Issue,
  include_labels: Option<&[String]>,
  exclude_labels: Option<&[String]>,
) -> bool {
  if let Some(include) = include_labels {
    if !include.is_empty() {
      let include_lower: std::collections::HashSet<String> =
        include.iter().map(|s| s.to_lowercase()).collect();
      if !issue
        .labels
        .iter()
        .any(|l| include_lower.contains(&l.to_lowercase()))
      {
        return false;
      }
    }
  }
  if let Some(exclude) = exclude_labels {
    if !exclude.is_empty() {
      let exclude_lower: std::collections::HashSet<String> =
        exclude.iter().map(|s| s.to_lowercase()).collect();
      if issue
        .labels
        .iter()
        .any(|l| exclude_lower.contains(&l.to_lowercase()))
      {
        return false;
      }
    }
  }
  true
}

#[cfg(test)]
mod tests {
  use super::*;

  fn issue(id: &str, labels: Vec<&str>) -> Issue {
    Issue {
      id: id.to_string(),
      identifier: format!("o/r#{}", id),
      title: "T".to_string(),
      description: None,
      priority: None,
      state: "open".to_string(),
      branch_name: None,
      url: None,
      labels: labels.into_iter().map(String::from).collect(),
      blocked_by: vec![],
      created_at: None,
      updated_at: None,
    }
  }

  #[test]
  fn no_filters_returns_all() {
    let issues = vec![issue("1", vec!["a"]), issue("2", vec![])];
    let out = apply_label_filters(issues.clone(), None, None);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].id, "1");
    assert_eq!(out[1].id, "2");
  }

  #[test]
  fn include_empty_returns_all() {
    let issues = vec![issue("1", vec!["a"])];
    let include: Vec<String> = vec![];
    let out = apply_label_filters(issues, Some(&include), None);
    assert_eq!(out.len(), 1);
  }

  #[test]
  fn include_retains_only_matching() {
    let issues = vec![
      issue("1", vec!["a"]),
      issue("2", vec!["b"]),
      issue("3", vec!["a", "b"]),
      issue("4", vec!["c"]),
    ];
    let include = vec!["a".to_string(), "b".to_string()];
    let out = apply_label_filters(issues, Some(&include), None);
    assert_eq!(out.len(), 3);
    assert!(out.iter().all(|i| i.id != "4"));
  }

  #[test]
  fn include_case_insensitive() {
    let issues = vec![issue("1", vec!["Label"])];
    let include = vec!["label".to_string()];
    let out = apply_label_filters(issues, Some(&include), None);
    assert_eq!(out.len(), 1);
  }

  #[test]
  fn exclude_drops_matching() {
    let issues = vec![
      issue("1", vec!["a"]),
      issue("2", vec!["x"]),
      issue("3", vec!["a", "x"]),
    ];
    let exclude = vec!["x".to_string()];
    let out = apply_label_filters(issues, None, Some(&exclude));
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].id, "1");
  }

  #[test]
  fn exclude_empty_returns_all() {
    let issues = vec![issue("1", vec!["x"])];
    let exclude: Vec<String> = vec![];
    let out = apply_label_filters(issues, None, Some(&exclude));
    assert_eq!(out.len(), 1);
  }

  #[test]
  fn include_then_exclude() {
    let issues = vec![
      issue("1", vec!["a"]),
      issue("2", vec!["a", "blocked"]),
      issue("3", vec!["b"]),
      issue("4", vec!["b", "blocked"]),
    ];
    let include = vec!["a".to_string(), "b".to_string()];
    let exclude = vec!["blocked".to_string()];
    let out = apply_label_filters(issues, Some(&include), Some(&exclude));
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].id, "1");
    assert_eq!(out[1].id, "3");
  }

  #[test]
  fn issue_empty_labels_excluded_by_include() {
    let issues = vec![issue("1", vec![]), issue("2", vec!["a"])];
    let include = vec!["a".to_string()];
    let out = apply_label_filters(issues, Some(&include), None);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].id, "2");
  }

  #[test]
  fn issue_passes_label_filters_none_passes() {
    let i = issue("1", vec!["any"]);
    assert!(issue_passes_label_filters(&i, None, None));
  }

  #[test]
  fn issue_passes_label_filters_include_fail() {
    let i = issue("1", vec!["other"]);
    let include = vec!["a".to_string(), "b".to_string()];
    assert!(!issue_passes_label_filters(&i, Some(&include), None));
  }

  #[test]
  fn issue_passes_label_filters_include_pass() {
    let i = issue("1", vec!["a"]);
    let include = vec!["a".to_string()];
    assert!(issue_passes_label_filters(&i, Some(&include), None));
  }

  #[test]
  fn issue_passes_label_filters_exclude_fail() {
    let i = issue("1", vec!["claimed"]);
    let exclude = vec!["claimed".to_string()];
    assert!(!issue_passes_label_filters(&i, None, Some(&exclude)));
  }

  #[test]
  fn issue_passes_label_filters_exclude_pass() {
    let i = issue("1", vec!["a"]);
    let exclude = vec!["claimed".to_string()];
    assert!(issue_passes_label_filters(&i, None, Some(&exclude)));
  }

  #[test]
  fn issue_passes_label_filters_retry_claim_label_released() {
    let i = issue("1", vec!["symphony-claimed"]);
    let exclude = vec!["symphony-claimed".to_string()];
    assert!(!issue_passes_label_filters(&i, None, Some(&exclude)));
  }
}
