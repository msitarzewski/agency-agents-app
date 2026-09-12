//! Child-process environment hygiene for the Linux AppImage build.
//!
//! linuxdeploy's `AppRun` wrapper exports `LD_LIBRARY_PATH` (and `PERLLIB`,
//! `QT_PLUGIN_PATH`, `GST_PLUGIN_SYSTEM_PATH*`) with the bundle's directories
//! **first** and the caller's original appended, keeping no separate copy of the
//! original. Every child process we spawn inherits that, so a *host* binary
//! resolves the bundle's Ubuntu 22.04 libraries instead of its own.
//!
//! On any host whose libcurl is newer than the bundled nghttp2 — Arch, and modern
//! Ubuntu too — `git` dies before it reaches the network:
//!
//! ```text
//! git-remote-https: symbol lookup error: libcurl.so.4: undefined symbol:
//!   nghttp2_option_set_no_rfc9113_leading_and_trailing_ws_validation
//! fatal: remote helper 'https' aborted session
//! ```
//!
//! which broke catalog setup, Pull and Check-for-updates for every AppImage user
//! (#94). `git` was only the visible victim: the tool-version probes and the
//! folder opener spawn host binaries through the same poisoned environment.
//!
//! We drop only entries that live under `$APPDIR`, so the host's own search path
//! survives, and do nothing at all when `$APPDIR` is unset — which is every
//! non-AppImage build, including all of macOS and Windows.

use std::ffi::OsString;
use std::path::Path;

/// Colon-separated path lists that `AppRun` prefixes with bundle directories.
const PATH_LIST_VARS: &[&str] = &[
    "LD_LIBRARY_PATH",
    "PERLLIB",
    "QT_PLUGIN_PATH",
    "GST_PLUGIN_SYSTEM_PATH",
    "GST_PLUGIN_SYSTEM_PATH_1_0",
    "XDG_DATA_DIRS",
    "GTK_PATH",
];

/// Single-value variables the AppImage points wholly at the bundle.
const BUNDLE_ONLY_VARS: &[&str] = &[
    "GIO_EXTRA_MODULES",
    "GSETTINGS_SCHEMA_DIR",
    "GDK_PIXBUF_MODULE_FILE",
    "GTK_IM_MODULE_FILE",
    "GTK_EXE_PREFIX",
    "GTK_DATA_PREFIX",
];

/// A change to apply to a child's environment: `Some(v)` sets, `None` removes.
type Override = (String, Option<OsString>);

/// Pure core, so the behaviour is testable without mutating the process environment.
fn overrides_from(appdir: Option<&Path>, get: impl Fn(&str) -> Option<OsString>) -> Vec<Override> {
    let Some(appdir) = appdir else {
        return Vec::new();
    };
    let mut out = Vec::new();

    for var in PATH_LIST_VARS {
        let Some(value) = get(var) else { continue };
        let kept: Vec<String> = value
            .to_string_lossy()
            .split(':')
            .filter(|entry| !entry.is_empty() && !Path::new(entry).starts_with(appdir))
            .map(str::to_string)
            .collect();

        if kept.is_empty() {
            out.push((var.to_string(), None));
        } else {
            let joined = OsString::from(kept.join(":"));
            // Only emit a change when we actually removed something, so a value
            // we did not poison is passed through untouched.
            if joined != value {
                out.push((var.to_string(), Some(joined)));
            }
        }
    }

    for var in BUNDLE_ONLY_VARS {
        let Some(value) = get(var) else { continue };
        if Path::new(&value).starts_with(appdir) {
            out.push((var.to_string(), None));
        }
    }

    out
}

/// Environment changes for the current process. Empty outside an AppImage.
pub fn bundle_env_overrides() -> Vec<Override> {
    let appdir = std::env::var_os("APPDIR").map(std::path::PathBuf::from);
    overrides_from(appdir.as_deref(), |k| std::env::var_os(k))
}

/// Strip the AppImage bundle's paths from a `std::process::Command`.
pub fn sanitize(cmd: &mut std::process::Command) {
    for (key, value) in bundle_env_overrides() {
        match value {
            Some(v) => cmd.env(key, v),
            None => cmd.env_remove(key),
        };
    }
}

/// Strip the AppImage bundle's paths from a `tokio::process::Command`.
pub fn sanitize_tokio(cmd: &mut tokio::process::Command) {
    for (key, value) in bundle_env_overrides() {
        match value {
            Some(v) => cmd.env(key, v),
            None => cmd.env_remove(key),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        move |k: &str| map.get(k).map(OsString::from)
    }

    #[test]
    fn no_appdir_means_no_changes() {
        // Every non-AppImage build: never touch the user's environment.
        let env = env_of(&[("LD_LIBRARY_PATH", "/opt/weird/lib")]);
        assert!(overrides_from(None, env).is_empty());
    }

    #[test]
    fn bundle_entries_are_dropped_and_host_entries_kept() {
        let appdir = PathBuf::from("/tmp/.mount_Agency");
        let env = env_of(&[(
            "LD_LIBRARY_PATH",
            "/tmp/.mount_Agency/usr/lib:/tmp/.mount_Agency/usr/lib/x86_64-linux-gnu:/opt/host/lib",
        )]);
        let got = overrides_from(Some(&appdir), env);
        assert_eq!(
            got,
            vec![(
                "LD_LIBRARY_PATH".to_string(),
                Some(OsString::from("/opt/host/lib"))
            )]
        );
    }

    #[test]
    fn a_wholly_bundled_list_is_removed() {
        let appdir = PathBuf::from("/tmp/.mount_Agency");
        let env = env_of(&[("LD_LIBRARY_PATH", "/tmp/.mount_Agency/usr/lib")]);
        assert_eq!(
            overrides_from(Some(&appdir), env),
            vec![("LD_LIBRARY_PATH".to_string(), None)]
        );
    }

    #[test]
    fn host_only_lists_are_left_alone() {
        // No change emitted at all, so we never rewrite a value we did not poison.
        let appdir = PathBuf::from("/tmp/.mount_Agency");
        let env = env_of(&[("XDG_DATA_DIRS", "/usr/local/share:/usr/share")]);
        assert!(overrides_from(Some(&appdir), env).is_empty());
    }

    #[test]
    fn xdg_data_dirs_keeps_the_host_half() {
        let appdir = PathBuf::from("/tmp/.mount_Agency");
        let env = env_of(&[("XDG_DATA_DIRS", "/tmp/.mount_Agency/usr/share:/usr/share")]);
        assert_eq!(
            overrides_from(Some(&appdir), env),
            vec![(
                "XDG_DATA_DIRS".to_string(),
                Some(OsString::from("/usr/share"))
            )]
        );
    }

    #[test]
    fn bundle_only_vars_are_removed_but_host_ones_are_not() {
        let appdir = PathBuf::from("/tmp/.mount_Agency");
        let env = env_of(&[
            (
                "GIO_EXTRA_MODULES",
                "/tmp/.mount_Agency/usr/lib/gio/modules",
            ),
            ("GTK_PATH", "/usr/lib/gtk-3.0"),
            ("GSETTINGS_SCHEMA_DIR", "/usr/share/glib-2.0/schemas"),
        ]);
        assert_eq!(
            overrides_from(Some(&appdir), env),
            vec![("GIO_EXTRA_MODULES".to_string(), None)]
        );
    }

    /// The behaviour that actually matters: a spawned child must not see the
    /// bundle path. This is the #94 regression in one assertion.
    #[cfg(unix)]
    #[test]
    fn a_spawned_child_does_not_inherit_the_bundle_path() {
        let appdir = "/tmp/.mount_AgencyTest";
        // SAFETY: single-threaded test process section; we set, spawn, and read back.
        unsafe {
            std::env::set_var("APPDIR", appdir);
            std::env::set_var("LD_LIBRARY_PATH", format!("{appdir}/usr/lib:/opt/host/lib"));
        }

        let mut poisoned = std::process::Command::new("sh");
        poisoned.args(["-c", "printf %s \"$LD_LIBRARY_PATH\""]);
        let before = String::from_utf8_lossy(&poisoned.output().unwrap().stdout).into_owned();
        assert!(
            before.contains(appdir),
            "test setup: child should start poisoned"
        );

        let mut clean = std::process::Command::new("sh");
        clean.args(["-c", "printf %s \"$LD_LIBRARY_PATH\""]);
        sanitize(&mut clean);
        let after = String::from_utf8_lossy(&clean.output().unwrap().stdout).into_owned();

        assert_eq!(after, "/opt/host/lib", "child kept the host path only");
        assert!(
            !after.contains(appdir),
            "child must not see the bundle path"
        );

        unsafe {
            std::env::remove_var("APPDIR");
            std::env::remove_var("LD_LIBRARY_PATH");
        }
    }
}
