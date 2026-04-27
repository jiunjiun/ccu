use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Root dir under which all JSONL files live.
/// Honors `CCU_PROJECTS_DIR` env for tests; otherwise `~/.claude/projects`.
pub fn scan_root() -> anyhow::Result<PathBuf> {
    if let Ok(dir) = std::env::var("CCU_PROJECTS_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let home = std::env::var("HOME").map_err(|_| anyhow::anyhow!("HOME is not set"))?;
    Ok(PathBuf::from(home).join(".claude").join("projects"))
}

/// Recursively find all `.jsonl` files beneath `root`, paired with each
/// file's mtime when available. The mtime is read from the same metadata
/// `WalkDir` already fetches for `file_type`, so threading it through here
/// avoids a second `stat` syscall in mtime-filtering callers.
pub fn discover_jsonl(root: &Path) -> Vec<(PathBuf, Option<DateTime<Utc>>)> {
    WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("jsonl"))
        .map(|e| {
            let mtime = e.metadata().ok().and_then(|m| m.modified().ok()).map(Into::into);
            (e.path().to_path_buf(), mtime)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;
    use tempfile::TempDir;

    /// Serializes any unit test that mutates process-global env.
    /// Cargo runs tests on multiple threads; set_var without a lock can race
    /// with any concurrent env read. Currently only `scan_root_honors_env_override`
    /// needs it — if more env-touching tests appear, have them lock the same mutex.
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    fn touch(dir: &Path, rel: &str) {
        let full = dir.join(rel);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, b"").unwrap();
    }

    #[test]
    fn discovers_jsonl_recursively() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), "a/file1.jsonl");
        touch(tmp.path(), "a/b/file2.jsonl");
        touch(tmp.path(), "a/ignored.txt");
        touch(tmp.path(), "a/b/other.json");

        let mut found: Vec<_> = discover_jsonl(tmp.path())
            .into_iter()
            .map(|(p, _)| p.strip_prefix(tmp.path()).unwrap().to_path_buf())
            .collect();
        found.sort();

        assert_eq!(
            found,
            vec![
                PathBuf::from("a/b/file2.jsonl"),
                PathBuf::from("a/file1.jsonl"),
            ]
        );
    }

    #[test]
    fn returns_empty_when_root_missing() {
        let fake = PathBuf::from("/tmp/definitely-not-a-real-dir-ccu-xyz");
        assert!(discover_jsonl(&fake).is_empty());
    }

    #[test]
    fn scan_root_honors_env_override() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("CCU_PROJECTS_DIR").ok();
        std::env::set_var("CCU_PROJECTS_DIR", tmp.path());
        let root = scan_root().unwrap();
        // Restore env before asserting so a failure doesn't leak state.
        match prev {
            Some(v) => std::env::set_var("CCU_PROJECTS_DIR", v),
            None => std::env::remove_var("CCU_PROJECTS_DIR"),
        }
        assert_eq!(root, tmp.path());
    }
}
