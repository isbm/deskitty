use crate::config::{Config, expand_tilde, set_show_desktop};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

fn parse(yaml: &str) -> Config {
    serde_yaml::from_str(yaml).expect("valid config")
}

fn temporary_config_path() -> PathBuf {
    static NEXT_TEST_FILE: AtomicUsize = AtomicUsize::new(0);

    let suffix = NEXT_TEST_FILE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("deskitty-config-test-{}-{suffix}.conf", std::process::id()))
}

#[test]
fn omitted_cwd_has_no_override() {
    let cfg = parse(
        r#"
tabs:
  - name: "Shell"
    shortcut: "s"
    command: ["bash"]
"#,
    );

    assert_eq!(cfg.tabs[0].cwd, None);
    assert_eq!(cfg.tabs[0].resolved_cwd(), None);
}

#[test]
fn parses_cwd_override() {
    let cfg = parse(
        r#"
tabs:
  - name: "Editor"
    shortcut: "e"
    command: ["editor"]
    cwd: "/tmp/project"
"#,
    );

    assert_eq!(cfg.tabs[0].cwd.as_deref(), Some(Path::new("/tmp/project")));
}

#[test]
fn expands_leading_tilde_to_home() {
    assert_eq!(expand_tilde(Path::new("~/work/project"), Path::new("/home/user")), PathBuf::from("/home/user/work/project"));
}

#[test]
fn expands_bare_tilde_to_home() {
    assert_eq!(expand_tilde(Path::new("~"), Path::new("/home/user")), PathBuf::from("/home/user"));
}

#[test]
fn leaves_other_paths_unchanged() {
    for path in ["/opt/project", "work/project", "~other/project"] {
        assert_eq!(expand_tilde(Path::new(path), Path::new("/home/user")), PathBuf::from(path));
    }
}

#[test]
fn actions_defaults_true_and_parses_false() {
    let cfg = parse("tabs: []\n");
    assert!(cfg.actions);

    let cfg = parse("actions: false\n");
    assert!(!cfg.actions);
}

#[test]
fn parses_debug_flag() {
    let cfg = parse("debug: true\n");
    assert!(cfg.debug);

    let cfg = parse("tabs: []\n");
    assert!(!cfg.debug);
}

#[test]
fn show_desktop_defaults_true_and_parses_false() {
    let cfg = parse("tabs: []\n");
    assert!(cfg.show_desktop);

    let cfg = parse("show_desktop: false\n");
    assert!(!cfg.show_desktop);
}

#[test]
fn persists_show_desktop_without_changing_other_settings() {
    let path = temporary_config_path();
    std::fs::write(&path, "tabs:\n  - name: Shell\n    shortcut: s\n    command: [bash]\nactions: false\nshow_desktop: true\n")
        .expect("write config");

    set_show_desktop(&path, false).expect("persist desktop setting");

    let content = std::fs::read_to_string(&path).expect("read config");
    assert!(content.contains("actions: false"));
    assert!(content.contains("show_desktop: false"));
    assert!(!content.contains("show_desktop: true"));
    std::fs::remove_file(path).expect("remove config");
}

#[test]
fn creates_missing_config_with_show_desktop_setting() {
    let path = temporary_config_path();

    set_show_desktop(&path, false).expect("persist desktop setting");

    assert_eq!(std::fs::read_to_string(&path).expect("read config"), "show_desktop: false\n");
    std::fs::remove_file(path).expect("remove config");
}

#[test]
fn default_invocations_select_default_conf() {
    let home = Path::new("/home/user");
    for invocation in [None, Some(""), Some("deskitty"), Some("./deskitty"), Some("/usr/bin/deskitty")] {
        assert_eq!(crate::config::selected_config_path(None, invocation.map(std::ffi::OsStr::new), home), home.join(".config/deskitty/default.conf"),);
    }
}

#[test]
fn explicit_config_yaml_overrides_foobar_profile() {
    assert_eq!(
        crate::config::selected_config_path(Some("config.yaml"), Some(std::ffi::OsStr::new("foobar")), Path::new("/home/user")),
        PathBuf::from("config.yaml"),
    );
}

#[test]
fn profile_invocations_select_basename_conf() {
    let home = Path::new("/home/user");
    for invocation in ["foobar", "./foobar", "/usr/local/bin/foobar"] {
        assert_eq!(
            crate::config::selected_config_path(None, Some(std::ffi::OsStr::new(invocation)), home),
            home.join(".config/deskitty/foobar.conf"),
        );
    }
}

#[test]
fn explicit_config_overrides_invocation_profile() {
    for invocation in ["deskitty", "foobar"] {
        assert_eq!(
            crate::config::selected_config_path(Some("custom/settings.yaml"), Some(std::ffi::OsStr::new(invocation)), Path::new("/home/user")),
            PathBuf::from("custom/settings.yaml"),
        );
    }
}

#[cfg(unix)]
#[test]
fn profile_names_preserve_non_utf8_bytes() {
    use std::os::unix::ffi::OsStrExt;

    let invocation = std::ffi::OsStr::from_bytes(b"/usr/bin/profile\xff");
    let expected = Path::new("/home/user/.config/deskitty").join(std::ffi::OsStr::from_bytes(b"profile\xff.conf"));
    assert_eq!(crate::config::selected_config_path(None, Some(invocation), Path::new("/home/user")), expected);
}
