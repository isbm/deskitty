use crate::config::{Config, expand_tilde};
use std::path::{Path, PathBuf};

fn parse(yaml: &str) -> Config {
    serde_yaml::from_str(yaml).expect("valid config")
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
