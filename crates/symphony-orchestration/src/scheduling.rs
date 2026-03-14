//! Poll scheduling helpers: candidate sort and retry delay (SPEC §8.2, §8.4).

use symphony_domain::Issue;

/// Sort issues for dispatch: priority ascending (1 = highest; null last), created_at oldest first (None last), identifier tie-break.
pub fn sort_for_dispatch(issues: &mut [Issue]) {
  tracing::trace!("sort_for_dispatch");
  issues.sort_by(|a, b| {
    let p = (a.priority.unwrap_or(i32::MAX)).cmp(&b.priority.unwrap_or(i32::MAX));
    if p != std::cmp::Ordering::Equal {
      return p;
    }
    let t = match (&a.created_at, &b.created_at) {
      (None, None) => std::cmp::Ordering::Equal,
      (None, Some(_)) => std::cmp::Ordering::Greater,
      (Some(_), None) => std::cmp::Ordering::Less,
      (Some(t1), Some(t2)) => t1.cmp(t2),
    };
    if t != std::cmp::Ordering::Equal {
      return t;
    }
    a.identifier.cmp(&b.identifier)
  });
}

/// Compute retry delay in ms. Continuation (normal exit): 1000 ms. Failure-driven: min(10_000 * 2^(attempt - 1), max_retry_backoff_ms).
pub fn retry_delay_ms(attempt: u32, max_retry_backoff_ms: u64, continuation: bool) -> u64 {
  tracing::trace!("retry_delay_ms");
  if continuation {
    return 1000;
  }
  let base = 10_000_u64;
  let delay = base.saturating_mul(2_u64.saturating_pow(attempt.saturating_sub(1)));
  delay.min(max_retry_backoff_ms)
}

#[cfg(test)]
mod tests {
  use super::*;
  use chrono::Utc;

  #[test]
  fn sort_for_dispatch_priority_ascending() {
    let mut issues = vec![
      Issue {
        id: "2".into(),
        identifier: "r#2".into(),
        title: "B".into(),
        description: None,
        priority: Some(2),
        state: "open".into(),
        branch_name: None,
        url: None,
        labels: vec![],
        blocked_by: vec![],
        created_at: None,
        updated_at: None,
      },
      Issue {
        id: "1".into(),
        identifier: "r#1".into(),
        title: "A".into(),
        description: None,
        priority: Some(1),
        state: "open".into(),
        branch_name: None,
        url: None,
        labels: vec![],
        blocked_by: vec![],
        created_at: None,
        updated_at: None,
      },
    ];
    sort_for_dispatch(&mut issues);
    assert_eq!(issues[0].id, "1");
    assert_eq!(issues[1].id, "2");
  }

  #[test]
  fn sort_for_dispatch_created_at_tie_break() {
    let t1 = Utc::now() - chrono::Duration::seconds(10);
    let t2 = Utc::now();
    let mut issues = vec![
      Issue {
        id: "new".into(),
        identifier: "r#2".into(),
        title: "T".into(),
        description: None,
        priority: None,
        state: "open".into(),
        branch_name: None,
        url: None,
        labels: vec![],
        blocked_by: vec![],
        created_at: Some(t2),
        updated_at: None,
      },
      Issue {
        id: "old".into(),
        identifier: "r#1".into(),
        title: "T".into(),
        description: None,
        priority: None,
        state: "open".into(),
        branch_name: None,
        url: None,
        labels: vec![],
        blocked_by: vec![],
        created_at: Some(t1),
        updated_at: None,
      },
    ];
    sort_for_dispatch(&mut issues);
    assert_eq!(issues[0].id, "old");
  }

  #[test]
  fn retry_delay_continuation() {
    assert_eq!(retry_delay_ms(1, 300_000, true), 1000);
    assert_eq!(retry_delay_ms(3, 300_000, true), 1000);
  }

  #[test]
  fn retry_delay_failure_attempt1() {
    assert_eq!(retry_delay_ms(1, 300_000, false), 10_000);
  }

  #[test]
  fn retry_delay_failure_attempt2() {
    assert_eq!(retry_delay_ms(2, 300_000, false), 20_000);
  }

  #[test]
  fn retry_delay_failure_capped() {
    assert_eq!(retry_delay_ms(5, 25_000, false), 25_000);
  }
}
