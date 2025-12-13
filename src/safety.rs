//! Safety module for root-only permanent deletion enforcement.
//!
//! This module implements the core safety invariant of safe-rm:
//! **Non-root users can NEVER permanently delete files via rm.**
//!
//! All permanent deletion operations must go through `require_root_for_permanent_deletion()`
//! which will return an error for non-root users.

use std::io::{Error, ErrorKind};

/// Trait for checking root privileges, enabling testability via mocking.
///
/// In production, use `SystemRootChecker` which calls the actual system APIs.
/// In tests, use `MockRootChecker` to simulate root/non-root scenarios.
pub trait RootChecker {
    /// Returns true if the current process has root privileges.
    fn is_root(&self) -> bool;
}

/// Production root checker using system APIs.
///
/// Uses `nix::unistd::geteuid().is_root()` for safe root detection.
pub struct SystemRootChecker;

impl RootChecker for SystemRootChecker {
    #[cfg(unix)]
    fn is_root(&self) -> bool {
        // Use nix's safe API instead of raw libc
        nix::unistd::geteuid().is_root()
    }

    #[cfg(not(unix))]
    fn is_root(&self) -> bool {
        // Non-Unix platforms: always return false (safe default)
        false
    }
}

/// Mock root checker for testing.
///
/// Allows tests to simulate both root and non-root scenarios without
/// requiring actual privilege escalation.
pub struct MockRootChecker {
    /// Whether to simulate root privileges.
    pub simulate_root: bool,
}

impl MockRootChecker {
    /// Create a mock that simulates a non-root user.
    pub fn non_root() -> Self {
        Self { simulate_root: false }
    }

    /// Create a mock that simulates a root user.
    pub fn root() -> Self {
        Self { simulate_root: true }
    }
}

impl RootChecker for MockRootChecker {
    fn is_root(&self) -> bool {
        self.simulate_root
    }
}

/// Error message shown when a non-root user attempts permanent deletion.
pub const ROOT_REQUIRED_MESSAGE: &str = "safe-rm: Only root can permanently delete files";

/// Require root privileges for permanent deletion operations.
///
/// This is the central safety gate that enforces the core invariant:
/// non-root users cannot permanently delete files.
///
/// # Arguments
///
/// * `checker` - A `RootChecker` implementation to verify privileges
///
/// # Returns
///
/// * `Ok(())` if the user has root privileges
/// * `Err` with `PermissionDenied` if the user is not root
///
/// # Examples
///
/// ```
/// use rip2::safety::{require_root_for_permanent_deletion, MockRootChecker};
///
/// // Non-root user attempting deletion
/// let checker = MockRootChecker::non_root();
/// assert!(require_root_for_permanent_deletion(&checker).is_err());
///
/// // Root user attempting deletion
/// let checker = MockRootChecker::root();
/// assert!(require_root_for_permanent_deletion(&checker).is_ok());
/// ```
pub fn require_root_for_permanent_deletion(checker: &impl RootChecker) -> Result<(), Error> {
    // Check if user has root privileges
    if checker.is_root() {
        Ok(())
    } else {
        // Deny permanent deletion for non-root users
        Err(Error::new(ErrorKind::PermissionDenied, ROOT_REQUIRED_MESSAGE))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_non_root_denies_deletion() {
        // Non-root user should be denied
        let checker = MockRootChecker::non_root();
        let result = require_root_for_permanent_deletion(&checker);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::PermissionDenied);
        assert!(err.to_string().contains("safe-rm"));
    }

    #[test]
    fn test_mock_root_allows_deletion() {
        // Root user should be allowed
        let checker = MockRootChecker::root();
        let result = require_root_for_permanent_deletion(&checker);

        assert!(result.is_ok());
    }

    #[test]
    fn test_system_root_checker_returns_bool() {
        // Just verify it returns a boolean without panicking
        let checker = SystemRootChecker;
        let _is_root: bool = checker.is_root();
    }
}
