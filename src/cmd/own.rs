use std::path::{Path, PathBuf};

use crate::spec::Ownership;
use crate::spec::io::{load_lock_or_empty, load_spec, save_lock};

/// Set or clear an explicit ownership decision in `.rtango/lock.yaml`.
///
/// - `rule_id = Some(id)`, `clear = false`: upsert `path -> id`.
/// - `rule_id = None`,     `clear = true` : remove any entry for `path`.
///
/// The spec is loaded to validate that `rule_id` references an existing rule.
/// Relative `path` values are resolved against `root` before storage, matching
/// the absolute-path keys the engine uses when resolving ownership.
pub fn exec(
    root: &Path,
    path: PathBuf,
    rule_id: Option<String>,
    clear: bool,
) -> anyhow::Result<()> {
    if !clear && rule_id.is_none() {
        anyhow::bail!("`rtango own` requires a rule id (or --clear)");
    }

    let spec = load_spec(root)?;
    if let Some(id) = &rule_id {
        if !spec.rules.iter().any(|r| r.id == *id) {
            anyhow::bail!("rule '{}' not found in spec", id);
        }
    }

    let abs = if path.is_absolute() {
        path
    } else {
        root.join(path)
    };

    let mut lock = load_lock_or_empty(root)?;
    lock.owners.retain(|o| o.path != abs);
    if let Some(id) = rule_id {
        lock.owners.push(Ownership {
            path: abs.clone(),
            rule_id: id.clone(),
        });
        println!("{} → {}", abs.display(), id);
    } else {
        println!("cleared ownership for {}", abs.display());
    }

    save_lock(root, &lock)?;
    Ok(())
}
