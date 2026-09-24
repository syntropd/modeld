//! Peer-credential authorization for SCM_RIGHTS descriptor handoff.
//!
//! Resolves a Unix group name to a numeric GID by scanning `/etc/group`,
//! then accepts or rejects incoming connections based on `SO_PEERCRED`
//! reported by the kernel.

use rustix::net::UCred;
use std::fs;
use std::path::Path;
use tracing::warn;

/// Resolves a group name to its numeric GID by scanning `/etc/group`.
///
/// Returns `None` if the group is not present in the file. Scanning is
/// intentionally cheap (the file is typically a few KiB) and avoids any
/// C-linkage FFI so this module stays pure Rust.
pub fn lookup_group_gid(group_name: &str) -> Option<u32> {
    parse_group_file(Path::new("/etc/group"), group_name)
}

/// Parses a single `/etc/group`-formatted line into its fourth colon-separated
/// field (the numeric gid) when the first field matches `wanted`.
fn parse_group_file(path: &Path, wanted: &str) -> Option<u32> {
    let contents = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            warn!("Cannot read {} for group lookup: {}", path.display(), e);
            return None;
        }
    };
    for line in contents.lines() {
        // Skip comments and blank lines.
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.split(':');
        let name = fields.next().unwrap_or("");
        if name != wanted {
            continue;
        }
        // fields: name : passwd : gid : members
        let _passwd = fields.next();
        // A malformed gid field on a matching line should not abort the
        // whole scan; skip this entry and continue searching later lines.
        let Some(gid_str) = fields.next() else {
            continue;
        };
        if let Ok(gid) = gid_str.parse::<u32>() {
            return Some(gid);
        }
    }
    None
}

/// Authorization policy applied to incoming FD-handoff connections.
///
/// The configured `trusted_group` authorizes any process whose primary
/// (effective) GID matches the group's numeric ID. Root (uid 0) is
/// always trusted; systemd socket activation hands off the FD before
/// the daemon drops privileges into the `modeld` user, so uid-0 must
/// be permitted for that path.
///
/// Supplementary-group membership is intentionally NOT consulted:
/// resolving it requires either NSS calls (libc `getgrouplist`, which
/// would re-introduce C-linkage) or `/proc/<pid>/status` parsing with
/// its own TOCTOU window. The primary-GID check covers the documented
/// threat model where the caller is the systemd-launched `modeld`
/// process or an explicit operator tool running as that group.
#[derive(Debug, Clone)]
pub struct TrustedGroupConfig {
    /// Group name whose members are authorized to receive model FDs.
    pub trusted_group: String,
    /// Optional override for the resolved numeric GID. When set, this
    /// value is used directly instead of scanning `/etc/group`. This is
    /// primarily a test seam so unit tests can drive the GID-matching
    /// branch without depending on the host's group database.
    pub trusted_gid_override: Option<u32>,
}

impl TrustedGroupConfig {
    /// Builds the policy from environment overrides or falls back to `modeld`.
    pub fn from_env_or_default() -> Self {
        let trusted_group = std::env::var("MODELD_TRUSTED_GROUP")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "modeld".to_string());
        Self {
            trusted_group,
            trusted_gid_override: None,
        }
    }

    /// Returns the numeric GID of the configured trusted group, if known.
    /// Honours `trusted_gid_override` first, then falls back to the
    /// `/etc/group` scan.
    pub fn trusted_gid(&self) -> Option<u32> {
        self.trusted_gid_override
            .or_else(|| lookup_group_gid(&self.trusted_group))
    }

    /// Pure authorization decision: returns true when a peer with the
    /// supplied uid/gid should be allowed to request a model FD.
    ///
    /// Split out so unit tests can drive the policy without consulting the
    /// host's `/etc/group` (the test seam lets a test inject `Some(gid)`
    /// directly rather than depend on the production gid lookup).
    pub fn check(&self, uid: u32, gid: u32) -> bool {
        // Root is always trusted; this matches systemd activation where the
        // service runs as `User=modeld` but root is allowed during activation
        // handoff in some configurations.
        if uid == 0 {
            return true;
        }
        match self.trusted_gid() {
            Some(trusted) => gid == trusted,
            None => false,
        }
    }

    /// Returns true when `peer` should be allowed to request a model FD.
    pub fn is_authorized(&self, peer: &UCred) -> bool {
        self.check(peer.uid.as_raw(), peer.gid.as_raw())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_group_file_finds_existing_group() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("group");
        std::fs::write(
            &path,
            "root:x:0:\nmodeld:x:1234:alice,bob\nadm:x:4:syslog\n",
        )
        .unwrap();
        assert_eq!(parse_group_file(&path, "modeld"), Some(1234));
        assert_eq!(parse_group_file(&path, "root"), Some(0));
        assert_eq!(parse_group_file(&path, "nonexistent"), None);
    }

    #[test]
    fn test_parse_group_file_skips_comments_and_blanks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("group");
        std::fs::write(&path, "# header\n\nmodeld:x:42:\n").unwrap();
        assert_eq!(parse_group_file(&path, "modeld"), Some(42));
    }

    #[test]
    fn test_parse_group_file_rejects_malformed_gid() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("group");
        std::fs::write(&path, "broken:x:not-a-number:\n").unwrap();
        assert_eq!(parse_group_file(&path, "broken"), None);
    }

    #[test]
    fn test_parse_group_file_skips_short_line_and_finds_later_entry() {
        // A matching line with FEWER than four fields would have caused
        // the previous `fields.next()?` implementation to abort the scan
        // with `None`; the fix continues to the next line. Use a 2-field
        // line so the divergence from the old code is unambiguous.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("group");
        std::fs::write(&path, "modeld:x\nmodeld:x:4242:\n").unwrap();
        assert_eq!(parse_group_file(&path, "modeld"), Some(4242));
    }

    #[test]
    fn test_is_authorized_trusts_root() {
        // Inject a non-zero trusted gid so the GID branch is reachable,
        // then assert root overrides it.
        let cfg = TrustedGroupConfig {
            trusted_group: "modeld".to_string(),
            trusted_gid_override: Some(7777),
        };
        let peer = make_cred(1, 0, 9999);
        assert!(cfg.is_authorized(&peer));
    }

    #[test]
    fn test_is_authorized_trusts_matching_gid() {
        let cfg = TrustedGroupConfig {
            trusted_group: "modeld".to_string(),
            trusted_gid_override: Some(7777),
        };
        let peer = make_cred(2, 1000, 7777);
        assert!(cfg.is_authorized(&peer));
    }

    #[test]
    fn test_is_authorized_rejects_non_matching_gid() {
        let cfg = TrustedGroupConfig {
            trusted_group: "modeld".to_string(),
            trusted_gid_override: Some(7777),
        };
        let peer = make_cred(3, 1000, 9999);
        assert!(!cfg.is_authorized(&peer));
    }

    #[test]
    fn test_is_authorized_rejects_when_no_trusted_gid_known() {
        // Override None simulates a host where the trusted group cannot be
        // resolved; every non-root peer must be rejected.
        let cfg = TrustedGroupConfig {
            trusted_group: "no-such-group".to_string(),
            trusted_gid_override: None,
        };
        let peer = make_cred(4, 1000, 1234);
        assert!(!cfg.is_authorized(&peer));
    }

    /// Constructs a synthetic `UCred` for unit testing.
    ///
    /// The `from_raw*` constructors are unsafe because the kernel does not
    /// validate the values; for unit tests where the values are obviously
    /// plausible (non-zero pid, sane uid/gid) the safety contract is
    /// trivially satisfied.
    fn make_cred(pid: i32, uid: u32, gid: u32) -> UCred {
        UCred {
            pid: unsafe { rustix::process::Pid::from_raw_unchecked(pid) },
            uid: unsafe { rustix::process::Uid::from_raw(uid) },
            gid: unsafe { rustix::process::Gid::from_raw(gid) },
        }
    }
}
