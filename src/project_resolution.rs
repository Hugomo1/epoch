use std::cmp::Ordering;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownProject {
    pub path: PathBuf,
    pub last_activity_epoch_secs: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SourcePriority {
    Alias = 0,
    GitRoot = 1,
    CwdAncestry = 2,
    KnownProject = 3,
    ArtifactColocation = 4,
}

#[derive(Debug, Clone)]
struct Candidate {
    path: PathBuf,
    source: SourcePriority,
    last_activity_epoch_secs: i64,
}

fn normalize(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn is_prefix(prefix: &Path, full: &Path) -> bool {
    full.starts_with(prefix)
}

fn find_git_root(cwd: &Path) -> Option<PathBuf> {
    let mut current = Some(cwd);
    while let Some(path) = current {
        if path.join(".git").exists() {
            return Some(path.to_path_buf());
        }
        current = path.parent();
    }
    None
}

fn compare_candidates(left: &Candidate, right: &Candidate) -> Ordering {
    left.source
        .cmp(&right.source)
        .then_with(|| {
            right
                .path
                .components()
                .count()
                .cmp(&left.path.components().count())
        })
        .then_with(|| {
            right
                .last_activity_epoch_secs
                .cmp(&left.last_activity_epoch_secs)
        })
        .then_with(|| {
            left.path
                .to_string_lossy()
                .cmp(&right.path.to_string_lossy())
        })
}

pub fn resolve_project_identity(
    cwd: &Path,
    alias_paths: &[PathBuf],
    known_projects: &[KnownProject],
    artifact_paths: &[PathBuf],
) -> Option<PathBuf> {
    let cwd = normalize(cwd);
    let mut candidates = Vec::<Candidate>::new();

    for alias in alias_paths {
        let alias_path = normalize(alias);
        candidates.push(Candidate {
            path: alias_path,
            source: SourcePriority::Alias,
            last_activity_epoch_secs: 0,
        });
    }

    if let Some(git_root) = find_git_root(&cwd) {
        candidates.push(Candidate {
            path: normalize(&git_root),
            source: SourcePriority::GitRoot,
            last_activity_epoch_secs: 0,
        });
    }

    candidates.push(Candidate {
        path: cwd.clone(),
        source: SourcePriority::CwdAncestry,
        last_activity_epoch_secs: 0,
    });

    for project in known_projects {
        let project_path = normalize(&project.path);
        if is_prefix(&project_path, &cwd) || is_prefix(&cwd, &project_path) {
            candidates.push(Candidate {
                path: project_path,
                source: SourcePriority::KnownProject,
                last_activity_epoch_secs: project.last_activity_epoch_secs,
            });
        }
    }

    for artifact in artifact_paths {
        let artifact_parent = artifact.parent().map(normalize);
        if let Some(parent) = artifact_parent
            && (is_prefix(&parent, &cwd) || is_prefix(&cwd, &parent))
        {
            candidates.push(Candidate {
                path: parent,
                source: SourcePriority::ArtifactColocation,
                last_activity_epoch_secs: 0,
            });
        }
    }

    candidates.sort_by(compare_candidates);
    candidates.first().map(|candidate| candidate.path.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("epoch-project-res-{label}-{unique}"));
        fs::create_dir_all(&root).expect("temp directory should be created");
        root
    }

    #[test]
    fn project_resolution_prefers_alias_over_git_root() {
        let root = temp_dir("alias-over-git");
        let git_root = root.join("repo");
        let nested = git_root.join("subdir");
        let alias = root.join("alias-target");
        fs::create_dir_all(git_root.join(".git")).expect("git marker should exist");
        fs::create_dir_all(&nested).expect("nested directory should exist");
        fs::create_dir_all(&alias).expect("alias path should exist");

        let resolved = resolve_project_identity(&nested, &[alias.clone()], &[], &[])
            .expect("project should resolve");
        assert_eq!(normalize(&resolved), normalize(&alias));
    }

    #[test]
    fn project_resolution_handles_symlink_and_nested_repo_cases() {
        let root = temp_dir("nested-git");
        let outer = root.join("outer");
        let inner = outer.join("inner");
        fs::create_dir_all(outer.join(".git")).expect("outer git should exist");
        fs::create_dir_all(inner.join(".git")).expect("inner git should exist");
        let cwd = inner.join("src");
        fs::create_dir_all(&cwd).expect("cwd should exist");

        let resolved = resolve_project_identity(&cwd, &[], &[], &[]).expect("resolution exists");
        assert_eq!(normalize(&resolved), normalize(&inner));

        #[cfg(unix)]
        {
            let link = root.join("inner-link");
            std::os::unix::fs::symlink(&inner, &link).expect("symlink should be created");
            let linked_cwd = link.join("src");
            let linked_resolved =
                resolve_project_identity(&linked_cwd, &[], &[], &[]).expect("resolution exists");
            assert_eq!(normalize(&linked_resolved), normalize(&inner));
        }
    }

    #[test]
    fn project_resolution_tiebreak_is_deterministic() {
        let root = temp_dir("deterministic");
        let cwd = root.join("workspace");
        fs::create_dir_all(&cwd).expect("cwd should exist");

        let alpha = KnownProject {
            path: root.join("workspace-alpha"),
            last_activity_epoch_secs: 50,
        };
        let beta = KnownProject {
            path: root.join("workspace-beta"),
            last_activity_epoch_secs: 50,
        };
        fs::create_dir_all(&alpha.path).expect("alpha should exist");
        fs::create_dir_all(&beta.path).expect("beta should exist");

        let first = resolve_project_identity(&cwd, &[], &[alpha.clone(), beta.clone()], &[])
            .expect("first resolution exists");
        let second = resolve_project_identity(&cwd, &[], &[beta, alpha], &[])
            .expect("second resolution exists");
        assert_eq!(first, second);
    }
}
