//! Environment variable resolution (SPEC §6.1).

use shellexpand::env_with_context_no_errors;

/// Expand `$VAR_NAME` and `${VAR_NAME}` from the environment.
/// Use for values that support indirection (e.g. `tracker.api_key`, `workspace.root`).
pub fn resolve_var(s: &str) -> String {
    env_with_context_no_errors(s, |key| std::env::var(key).ok()).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_var_passthrough() {
        assert_eq!(resolve_var("hello"), "hello");
        assert_eq!(resolve_var(""), "");
    }

    #[test]
    fn resolve_var_expands_env() {
        std::env::set_var("SYMPHONY_TEST_VAR", "secret");
        let out = resolve_var("token=$SYMPHONY_TEST_VAR");
        std::env::remove_var("SYMPHONY_TEST_VAR");
        assert_eq!(out, "token=secret");
    }
}
