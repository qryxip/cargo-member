use anyhow::Context as _;
use log::debug;
use serde::de::DeserializeOwned;
use std::path::Path;

pub(crate) fn read_toml<T: DeserializeOwned, P: AsRef<Path>>(path: P) -> anyhow::Result<T> {
    let path = path.as_ref();
    let toml = toml::from_str(&read_to_string(path)?)
        .with_context(|| format!("failed to parse the TOML file at {}", path.display()))?;
    debug!("Read the TOML file at {}", path.display());
    Ok(toml)
}

pub(crate) fn read_toml_edit(path: impl AsRef<Path>) -> anyhow::Result<toml_edit::Document> {
    let path = path.as_ref();
    let edit = read_to_string(path)?
        .parse()
        .with_context(|| format!("failed to parse the TOML file at {}", path.display()))?;
    debug!("Read the TOML file at {}", path.display());
    Ok(edit)
}

fn read_to_string(path: &Path) -> anyhow::Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))
}

pub(crate) fn write(
    path: impl AsRef<Path>,
    contents: impl AsRef<[u8]>,
    dry_run: bool,
) -> anyhow::Result<()> {
    let path = path.as_ref();
    if !dry_run {
        std::fs::write(path, contents)
            .with_context(|| format!("failed to write {}", path.display()))?;
    }
    debug!(
        "{}Wrote {}",
        if dry_run { "[dry-run] " } else { "" },
        path.display(),
    );
    Ok(())
}

pub(crate) fn copy(
    src: impl AsRef<Path>,
    dst: impl AsRef<Path>,
    dry_run: bool,
) -> anyhow::Result<()> {
    let (src, dst) = (src.as_ref(), dst.as_ref());
    if !dry_run {
        std::fs::copy(src, dst).with_context(|| {
            format!("failed to copy `{}` to `{}`", src.display(), dst.display())
        })?;
    }
    debug!(
        "{}Copied {} to {}",
        if dry_run { "[dry-run] " } else { "" },
        src.display(),
        dst.display(),
    );
    Ok(())
}

pub(crate) fn create_dir_all(path: impl AsRef<Path>, dry_run: bool) -> anyhow::Result<()> {
    let path = path.as_ref();
    if !dry_run {
        std::fs::create_dir_all(path)
            .with_context(|| format!("failed to create `{}`", path.display()))?;
    }
    debug!(
        "{}Created {}",
        if dry_run { "[dry-run] " } else { "" },
        path.display(),
    );
    Ok(())
}

pub(crate) fn remove_dir_all(path: impl AsRef<Path>, dry_run: bool) -> anyhow::Result<()> {
    let path = path.as_ref();
    if !dry_run {
        remove_dir_all::remove_dir_all(path)
            .with_context(|| format!("failed to remove `{}`", path.display()))?;
    }
    debug!(
        "{}Removed {}",
        if dry_run { "[dry-run] " } else { "" },
        path.display(),
    );
    Ok(())
}
