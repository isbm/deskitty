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
