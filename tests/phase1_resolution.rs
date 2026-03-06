use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use epoch::project_resolution::{KnownProject, resolve_project_identity};

fn temp_dir(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("epoch-phase1-resolution-{label}-{unique}"));
    fs::create_dir_all(&root).expect("temp directory should be created");
    root
}

#[test]
fn project_resolution_prefers_alias_over_git_root() {
    let root = temp_dir("alias-over-git");
    let repo = root.join("repo");
    let cwd = repo.join("train");
    let alias = root.join("preferred-alias");

    fs::create_dir_all(repo.join(".git")).expect("git marker should exist");
    fs::create_dir_all(&cwd).expect("cwd should exist");
    fs::create_dir_all(&alias).expect("alias should exist");

    let resolved = resolve_project_identity(&cwd, &[alias.clone()], &[], &[])
        .expect("resolution should exist");
    assert_eq!(resolved, alias);
}

#[test]
fn project_resolution_handles_symlink_and_nested_repo_cases() {
    let root = temp_dir("symlink-nested");
    let outer = root.join("outer");
    let inner = outer.join("inner");
    let cwd = inner.join("run");

    fs::create_dir_all(outer.join(".git")).expect("outer git marker should exist");
    fs::create_dir_all(inner.join(".git")).expect("inner git marker should exist");
    fs::create_dir_all(&cwd).expect("cwd should exist");

    let direct = resolve_project_identity(&cwd, &[], &[], &[]).expect("resolution should exist");
    assert_eq!(direct, inner);

    #[cfg(unix)]
    {
        let link = root.join("inner-link");
        std::os::unix::fs::symlink(&inner, &link).expect("symlink should be created");
        let linked_cwd = link.join("run");
        let via_link =
            resolve_project_identity(&linked_cwd, &[], &[], &[]).expect("resolution exists");
        assert_eq!(via_link, inner);
    }
}

#[test]
fn project_resolution_tiebreak_is_deterministic() {
    let root = temp_dir("tiebreak");
    let cwd = root.join("workspace");
    fs::create_dir_all(&cwd).expect("cwd should exist");

    let alpha = KnownProject {
        path: root.join("workspace-alpha"),
        last_activity_epoch_secs: 42,
    };
    let beta = KnownProject {
        path: root.join("workspace-beta"),
        last_activity_epoch_secs: 42,
    };
    fs::create_dir_all(&alpha.path).expect("alpha should exist");
    fs::create_dir_all(&beta.path).expect("beta should exist");

    let first = resolve_project_identity(&cwd, &[], &[alpha.clone(), beta.clone()], &[])
        .expect("first resolution should exist");
    let second = resolve_project_identity(&cwd, &[], &[beta, alpha], &[])
        .expect("second resolution should exist");
    assert_eq!(first, second);
}
