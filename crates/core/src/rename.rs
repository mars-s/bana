//! The product was called TanaCraft before it was Bana. Notes written then live in `~/TanaCraft`
//! (or `$TANACRAFT_HOME`) as `graph.tanacraft`, with an edit log (`graph.tanacraft.log`) and a
//! folder of attached files (`graph.tanacraft.files`). On first start the app copies them over:
//!
//! - [`migrate_home`]: `~/TanaCraft` to `~/Bana` when `~/Bana` does not exist yet;
//! - [`migrate_file`]: `x.tanacraft` (and its log and files) to `x.bana` beside it, when a `.bana`
//!   file is asked for that does not exist (an explicit `--file`, the phone's Documents folder, or a
//!   data folder named by the old variable).
//!
//! Always a copy into a staging place, checked to load, then moved into place: the old files are
//! never changed or removed, and a copy that does not load is dropped (the error says so).

use std::path::{Path, PathBuf};

use crate::{Error, Graph, Result};

/// The old data folder's name, file extension and variable.
pub const OLD_DIR: &str = "TanaCraft";
pub const OLD_EXT: &str = "tanacraft";
pub const OLD_HOME_VAR: &str = "TANACRAFT_HOME";

fn io(p: &Path) -> impl Fn(std::io::Error) -> Error + '_ {
    move |e| Error::Io(format!("{}: {e}", p.display()))
}

/// `name` with the old extension changed to the new one (`graph.tanacraft.log` to `graph.bana.log`).
fn renamed(name: &str) -> String {
    name.replace(&format!(".{OLD_EXT}"), ".bana")
}

/// Copies `from` to `to` (a file or a whole folder); folder entries get their new names.
fn copy(from: &Path, to: &Path) -> Result<()> {
    let meta = std::fs::symlink_metadata(from).map_err(io(from))?;
    if meta.is_dir() {
        std::fs::create_dir_all(to).map_err(io(to))?;
        for e in std::fs::read_dir(from).map_err(io(from))? {
            let e = e.map_err(io(from))?;
            let name = e.file_name().to_string_lossy().to_string();
            // A running app's announcement is not data, and a half-written temp file is not either.
            if name.ends_with(".live") || name.ends_with(".tmp") {
                continue;
            }
            copy(&e.path(), &to.join(renamed(&name)))?;
        }
    } else if meta.is_file() {
        std::fs::copy(from, to).map_err(io(from))?;
    }
    Ok(())
}

/// Checks a copied graph loads (with its edit log).
fn verify(graph: &Path) -> Result<()> {
    if graph.exists() {
        Graph::load(graph)
            .map(drop)
            .map_err(|e| Error::Io(format!("the copy of your notes in {} does not load ({e}); the old files are untouched", graph.display())))?;
    }
    Ok(())
}

/// The data folder of old: `$TANACRAFT_HOME`, else `~/TanaCraft`.
pub fn old_home() -> PathBuf {
    if let Some(d) = std::env::var_os(OLD_HOME_VAR).filter(|d| !d.is_empty()) {
        return PathBuf::from(d);
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_default();
    home.join(OLD_DIR)
}

/// Copies the old data folder to the new one (see the module docs). Returns whether it did.
pub fn migrate_home() -> Result<bool> {
    migrate_home_to(&old_home(), &crate::data_dir())
}

/// [`migrate_home`] with the folders given. A new folder that exists but holds no graph yet (a
/// token made by a tool that ran first) still gets the old notes; the old token replaces its own.
pub fn migrate_home_to(old: &Path, new: &Path) -> Result<bool> {
    if !new.exists() {
        return migrate_dir(old, new);
    }
    let has_notes = ["graph.bana", "graph.json"].iter().any(|g| new.join(g).exists());
    if has_notes || old == new || !old.is_dir() {
        // Its own folder (or the old one named by the old variable): only the graph file may need moving.
        return migrate_file(&new.join("graph.bana"));
    }
    let parent = new.parent().unwrap_or(Path::new("."));
    let staging = parent.join(format!(".{}-merging-{}", new.file_name().unwrap_or_default().to_string_lossy(), std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    if !migrate_dir(old, &staging)? {
        return Ok(false);
    }
    for e in std::fs::read_dir(&staging).map_err(io(&staging))?.flatten() {
        let to = new.join(e.file_name());
        if to.exists() && e.file_name() != "token" {
            continue;
        }
        std::fs::rename(e.path(), &to).map_err(io(&to))?;
    }
    let _ = std::fs::remove_dir_all(&staging);
    Ok(true)
}

/// Copies folder `old` to `new` (which must not exist), files renamed, the graph checked.
pub fn migrate_dir(old: &Path, new: &Path) -> Result<bool> {
    if new.exists() || !old.is_dir() {
        return Ok(false);
    }
    let parent = new.parent().unwrap_or(Path::new("."));
    let staging = parent.join(format!(".{}-moving-{}", new.file_name().unwrap_or_default().to_string_lossy(), std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    let r = copy(old, &staging).and_then(|_| verify(&staging.join("graph.bana")));
    if let Err(e) = r {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }
    std::fs::rename(&staging, new).map_err(io(new))?;
    Ok(true)
}

/// For `x.bana` that does not exist: copies `x.tanacraft`, its edit log and files folder beside it
/// (see the module docs). Returns whether it did.
pub fn migrate_file(path: &Path) -> Result<bool> {
    if path.exists() || path.extension().is_none_or(|e| e != "bana") {
        return Ok(false);
    }
    let old = path.with_extension(OLD_EXT);
    if !old.is_file() {
        return Ok(false);
    }
    let mut copied = Vec::new();
    let mut step = || -> Result<()> {
        // The graph goes last, under a temporary name, so a half copy is never taken for the graph.
        for (from, to) in [(crate::graph::log_path(&old), crate::graph::log_path(path)), (crate::files_dir(&old), crate::files_dir(path))] {
            if from.exists() && !to.exists() {
                copy(&from, &to)?;
                copied.push(to);
            }
        }
        let tmp = path.with_extension("bana-moving");
        std::fs::copy(&old, &tmp).map_err(io(&old))?;
        copied.push(tmp.clone());
        // Checked under the graph's own name (its log is found beside it).
        std::fs::rename(&tmp, path).map_err(io(path))?;
        copied.push(path.to_path_buf());
        verify(path)
    };
    match step() {
        Ok(()) => Ok(true),
        Err(e) => {
            for p in copied {
                let _ = if p.is_dir() { std::fs::remove_dir_all(&p) } else { std::fs::remove_file(&p) };
            }
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("bana-rename-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// An old data folder: a graph with an edit made after its last save (in the log), a token,
    /// attached files and a live file.
    fn old_folder(dir: &Path) -> crate::NodeId {
        let g_path = dir.join("graph.tanacraft");
        let mut g = crate::sample::sample(crate::Date::new(2026, 10, 10).unwrap()).unwrap();
        g.save(&g_path).unwrap();
        let n = g.create(g.sys.library, None, "typed after the last save").unwrap();
        g.append_log(&g_path).unwrap();
        std::fs::write(dir.join("token"), "0123456789abcdef0123456789abcdef").unwrap();
        std::fs::create_dir_all(dir.join("graph.tanacraft.files")).unwrap();
        std::fs::write(dir.join("graph.tanacraft.files/photo.png"), b"png").unwrap();
        std::fs::write(dir.join("graph.tanacraft.live"), "{}").unwrap();
        n
    }

    fn listing(dir: &Path) -> Vec<(String, Vec<u8>)> {
        let mut out = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                if e.path().is_dir() {
                    stack.push(e.path());
                } else {
                    out.push((e.path().display().to_string(), std::fs::read(e.path()).unwrap()));
                }
            }
        }
        out.sort();
        out
    }

    #[test]
    fn the_old_folder_is_copied_renamed_and_left_untouched() {
        let root = tmp("home");
        let old = root.join("TanaCraft");
        std::fs::create_dir_all(&old).unwrap();
        let n = old_folder(&old);
        let before = listing(&old);
        let new = root.join("Bana");
        assert!(migrate_dir(&old, &new).unwrap());
        assert_eq!(listing(&old), before, "the old folder is untouched");
        let g = Graph::load(&new.join("graph.bana")).unwrap();
        assert_eq!(g.name(n), "typed after the last save", "the edit log came along");
        assert!(new.join("graph.bana.log").exists());
        assert_eq!(std::fs::read(new.join("graph.bana.files/photo.png")).unwrap(), b"png");
        assert!(new.join("token").exists());
        assert!(!new.join("graph.bana.live").exists());
        // Once there, nothing happens again.
        assert!(!migrate_dir(&old, &new).unwrap());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_new_folder_with_only_a_token_still_gets_the_old_notes() {
        let root = tmp("merge");
        let old = root.join("TanaCraft");
        std::fs::create_dir_all(&old).unwrap();
        let n = old_folder(&old);
        let new = root.join("Bana");
        std::fs::create_dir_all(&new).unwrap();
        std::fs::write(new.join("token"), "made by a tool that ran first").unwrap();
        assert!(migrate_home_to(&old, &new).unwrap());
        assert_eq!(Graph::load(&new.join("graph.bana")).unwrap().name(n), "typed after the last save");
        assert_eq!(std::fs::read(new.join("token")).unwrap(), std::fs::read(old.join("token")).unwrap(), "the old token stays the token");
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 2, "no staging folder left");
        // Notes there now: never again.
        assert!(!migrate_home_to(&old, &new).unwrap());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_damaged_old_graph_is_not_taken_and_nothing_is_left_behind() {
        let root = tmp("bad");
        let old = root.join("TanaCraft");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("graph.tanacraft"), b"\x00\x01 not a document").unwrap();
        let new = root.join("Bana");
        assert!(migrate_dir(&old, &new).is_err());
        assert!(!new.exists());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1, "no staging folder left");
        assert!(old.join("graph.tanacraft").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_graph_file_is_copied_beside_itself() {
        let dir = tmp("file");
        let n = old_folder(&dir);
        let path = dir.join("graph.bana");
        assert!(migrate_file(&path).unwrap());
        assert_eq!(Graph::load(&path).unwrap().name(n), "typed after the last save");
        assert!(dir.join("graph.tanacraft").exists() && dir.join("graph.tanacraft.log").exists());
        assert!(dir.join("graph.bana.files/photo.png").exists());
        assert!(!migrate_file(&path).unwrap(), "only once");
        // Nothing to move: nothing happens.
        assert!(!migrate_file(&dir.join("other.bana")).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
