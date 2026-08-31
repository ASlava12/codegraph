//! A repository this machine can read.
//!
//! Every command takes a path. Given a URL it takes a clone instead, made
//! once under the cache directory and reused afterwards. Only git does the
//! talking: the tool already required to have the source at all, with the
//! credentials the user has already set up for it.

use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, bail};
use codegraph_storage::default_cache_dir;

/// Whether a target names a repository to fetch rather than a directory to
/// read. A Windows path starts `C:\`, which is not a scheme.
pub(crate) fn is_remote(target: &str) -> bool {
    target.starts_with("https://")
        || target.starts_with("http://")
        || target.starts_with("ssh://")
        || target.starts_with("git://")
        // git clones from a path as readily as from a host, and saying so
        // is what lets this be exercised without reaching anything.
        || target.starts_with("file://")
        || target.starts_with("git@")
}

/// The directory a target stands for: itself when it is a path, and a clone
/// when it is a URL.
///
/// The clone is not refreshed on a later run. A scan that quietly fetched
/// would answer about code the caller never asked for, and `git -C <dir>
/// pull` is the command for when they want the new one.
pub(crate) fn repository_path(target: &str) -> Result<PathBuf> {
    if !is_remote(target) {
        return Ok(PathBuf::from(target));
    }
    let destination = clone_destination(target)?;
    if destination.join(".git").is_dir() {
        return Ok(destination);
    }
    if destination.exists() {
        bail!(
            "{} exists but is not a git clone; remove it or scan it as a path",
            destination.display()
        );
    }
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    eprintln!("cloning {target} into {}", destination.display());
    let status = Command::new("git")
        .arg("clone")
        .arg(target)
        .arg(&destination)
        .status()
        .with_context(|| format!("failed to run git clone for {target}"))?;
    if !status.success() {
        bail!("git clone of {target} failed");
    }
    Ok(destination)
}

/// Where a URL's clone lives: under the cache, by the host and path it names,
/// so the same URL is the same directory every time and two repositories of
/// the same name from different owners do not collide.
fn clone_destination(target: &str) -> Result<PathBuf> {
    let (host, path) = split_remote(target)?;
    let mut destination = default_cache_dir().join("repos").join(host);
    for segment in path.split('/').filter(|segment| !segment.is_empty()) {
        if segment == ".." || segment.contains('\\') {
            bail!("{target} is not a repository URL this can place on disk");
        }
        destination.push(segment);
    }
    Ok(destination)
}

/// The host and the path a remote names, in either form git accepts:
/// `https://github.com/owner/repo.git` and `git@github.com:owner/repo.git`
/// are the same repository.
fn split_remote(target: &str) -> Result<(&str, &str)> {
    let rest = target
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(target);
    // `git@host:owner/repo` -- the colon separates host from path, and the
    // user before `@` is not part of either.
    let rest = rest.split_once('@').map(|(_, rest)| rest).unwrap_or(rest);
    // Whichever separator comes first: `git@host:owner/repo` puts the colon
    // before the slash, and splitting on the slash would call
    // `host:owner` the host.
    let cut = rest
        .find(['/', ':'])
        .with_context(|| format!("{target} names no repository path"))?;
    let (host, path) = (&rest[..cut], &rest[cut + 1..]);
    // `file:///srv/repo.git` names no host at all; its clone still needs a
    // place, and every local remote can share one.
    let host = if host.is_empty() { "local" } else { host };
    let path = path.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    if host.is_empty() || path.is_empty() {
        bail!("{target} names no repository path");
    }
    Ok((host, path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_url_and_its_ssh_form_name_the_same_directory() {
        let https = clone_destination("https://github.com/ASlava12/codegraph").unwrap();
        let with_git = clone_destination("https://github.com/ASlava12/codegraph.git").unwrap();
        let ssh = clone_destination("git@github.com:ASlava12/codegraph.git").unwrap();
        assert_eq!(https, with_git);
        assert_eq!(https, ssh, "one repository, one clone");
        assert!(
            https.ends_with("repos/github.com/ASlava12/codegraph"),
            "placed by host and owner so two repos of a name do not collide: {}",
            https.display()
        );
    }

    #[test]
    fn a_path_is_not_a_remote() {
        assert!(!is_remote("."));
        assert!(!is_remote("/Users/x/project"));
        assert!(!is_remote("../sibling"));
        assert!(is_remote("https://github.com/o/r"));
        assert!(is_remote("git@github.com:o/r.git"));
    }

    #[test]
    fn a_url_that_names_no_repository_is_refused() {
        assert!(clone_destination("https://github.com").is_err());
        assert!(clone_destination("https://github.com/owner/../etc").is_err());
    }
}
