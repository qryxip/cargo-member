pub mod cli;
mod fs;

use anyhow::{anyhow, bail, ensure, Context as _};
use ignore::WalkBuilder;
use std::{fmt::Display, io, iter, path::Path, str};
use termcolor::{ColorSpec, WriteColor};

pub fn include<P: AsRef<Path>, W: WriteColor>(
    workspace_root: &Path,
    paths: &[P],
    force: bool,
    dry_run: bool,
    mut stderr: W,
) -> anyhow::Result<()> {
    for path in iter::once(workspace_root).chain(paths.iter().map(|p| p.as_ref())) {
        ensure_absolute(path)?;
    }
    let modified = paths.iter().try_fold(false, |acc, path| {
        let path = path.as_ref();
        if !(force || path.join("Cargo.toml").exists()) {
            return Err(
                anyhow!("`{}` does not exist", path.join("Cargo.toml").display()).context(format!(
                    "`{}` does not seem to be a package. enable `--force` to add",
                    path.display(),
                )),
            );
        }
        modify_members(
            workspace_root,
            Some(path),
            None,
            None,
            Some(path),
            dry_run,
            &mut stderr,
        )
        .map(|p| acc | p)
    })?;
    if !modified {
        stderr.warn("`workspace` unchanged")?;
    }
    if dry_run {
        stderr.warn("not modifying the manifest due to dry run")?;
    }
    Ok(())
}

pub fn exclude<P: AsRef<Path>, W: WriteColor>(
    workspace_root: &Path,
    paths: &[P],
    dry_run: bool,
    mut stderr: W,
) -> anyhow::Result<()> {
    for path in iter::once(workspace_root).chain(paths.iter().map(|p| p.as_ref())) {
        ensure_absolute(path)?;
    }
    let modified = paths.iter().try_fold(false, |acc, path| {
        let path = path.as_ref();
        modify_members(
            workspace_root,
            None,
            Some(path),
            Some(path),
            None,
            dry_run,
            &mut stderr,
        )
        .map(|p| acc | p)
    })?;
    if !modified {
        stderr.warn("`workspace` unchanged")?;
    }
    if dry_run {
        stderr.warn("not modifying the manifest due to dry run")?;
    }
    Ok(())
}

pub fn cp<W: WriteColor>(
    workspace_root: &Path,
    src: &Path,
    dst: &Path,
    dry_run: bool,
    mut stderr: W,
) -> anyhow::Result<()> {
    for path in &[workspace_root, src, dst] {
        ensure_absolute(path)?;
    }

    let dst = if dst.exists() {
        dst.join(src.file_name().expect("should be absolute"))
    } else {
        dst.to_owned()
    };

    let mut cargo_toml = crate::fs::read_toml_edit(src.join("Cargo.toml"))
        .with_context(|| format!("`{}` does not seem to be a package", src.display()))?;
    if let Some(package) = cargo_toml["package"].as_table_mut() {
        package.remove("workspace");
    }

    stderr.status_with_color(
        "Copying",
        format!("`{}` to `{}`", src.display(), dst.display()),
        termcolor::Color::Green,
    )?;

    let src_root = src;
    for src in WalkBuilder::new(src_root).hidden(false).build() {
        match src {
            Ok(src) => {
                let src = src.path();
                if !(src.is_dir()
                    || src == src_root.join("Cargo.toml")
                    || src.starts_with(src_root.join(".git")))
                {
                    let dst = dst.join(src.strip_prefix(src_root)?);
                    if let Some(parent) = dst.parent() {
                        if !parent.exists() {
                            crate::fs::create_dir_all(parent, dry_run)?;
                        }
                    }
                    crate::fs::copy(src, dst, dry_run)?;
                }
            }
            Err(err) => stderr.warn(err)?,
        }
    }

    crate::fs::write(dst.join("Cargo.toml"), cargo_toml.to_string(), dry_run)?;
    if dry_run {
        stderr.warn("not copying due to dry run")?;
    }
    Ok(())
}

pub fn rm<P: AsRef<Path>, W: WriteColor>(
    workspace_root: &Path,
    paths: &[P],
    force: bool,
    dry_run: bool,
    mut stderr: W,
) -> anyhow::Result<()> {
    for path in iter::once(workspace_root).chain(paths.iter().map(|p| p.as_ref())) {
        ensure_absolute(path)?;
    }

    let modified = paths.iter().try_fold(false, |acc, path| {
        let path = path.as_ref();
        if !(force || path.join("Cargo.toml").exists()) {
            return Err(
                anyhow!("`{}` does not exist", path.join("Cargo.toml").display()).context(format!(
                    "`{}` does not seem to be a package. enable `--force` to remove",
                    path.display(),
                )),
            );
        }
        stderr.status_with_color(
            "Removing",
            format!("directory `{}`", path.display()),
            termcolor::Color::Red,
        )?;
        crate::fs::remove_dir_all(path, dry_run)?;
        modify_members(
            workspace_root,
            None,
            None,
            Some(path),
            Some(path),
            dry_run,
            &mut stderr,
        )
        .map(|p| acc | p)
    })?;
    if !modified {
        stderr.warn("`workspace` unchanged")?;
    }
    if dry_run {
        stderr.warn("not modifying the manifest due to dry run")?;
    }
    Ok(())
}

pub fn mv<W: WriteColor>(
    workspace_root: &Path,
    src: &Path,
    dst: &Path,
    dry_run: bool,
    mut stderr: W,
) -> anyhow::Result<()> {
    cp(workspace_root, src, dst, dry_run, &mut stderr)?;
    rm(workspace_root, &[src], false, dry_run, &mut stderr)
}

fn ensure_absolute(path: &Path) -> anyhow::Result<()> {
    ensure!(path.is_absolute(), "must be absolute: {}", path.display());
    Ok(())
}

fn modify_members<'a>(
    workspace_root: &Path,
    add_to_workspace_members: Option<&'a Path>,
    add_to_workspace_exclude: Option<&'a Path>,
    rm_from_workspace_members: Option<&'a Path>,
    rm_from_workspace_exclude: Option<&'a Path>,
    dry_run: bool,
    mut stderr: impl WriteColor,
) -> anyhow::Result<bool> {
    if [
        add_to_workspace_members,
        add_to_workspace_exclude,
        rm_from_workspace_members,
        rm_from_workspace_exclude,
    ]
    .iter()
    .flatten()
    .any(|&p| p == workspace_root)
    {
        bail!("`{}` is the workspace root", workspace_root.display());
    }

    let manifest_path = workspace_root.join("Cargo.toml");
    let mut cargo_toml = crate::fs::read_toml_edit(&manifest_path)?;
    let orig = cargo_toml.to_string();

    for (field, add, rm) in &[
        (
            "members",
            add_to_workspace_members,
            rm_from_workspace_members,
        ),
        (
            "exclude",
            add_to_workspace_exclude,
            rm_from_workspace_exclude,
        ),
    ] {
        let relative_to_root = |path: &'a Path| -> _ {
            let path = path.strip_prefix(workspace_root).unwrap_or(path);
            path.to_str()
                .with_context(|| format!("{:?} is not valid UTF-8 path", path))
        };

        let same_paths = |value: &toml_edit::Value, target: &str| -> _ {
            value.as_str().map_or(false, |s| {
                workspace_root.join(s) == workspace_root.join(target)
            })
        };

        let array = cargo_toml["workspace"][field]
            .or_insert(toml_edit::value(toml_edit::Array::default()))
            .as_array_mut()
            .with_context(|| format!("`workspace.{}` must be an array", field))?;
        if let Some(add) = *add {
            let add = relative_to_root(add)?;
            if array.iter().all(|m| !same_paths(m, add)) {
                if !dry_run {
                    array.push(add);
                }
                stderr.status_with_color(
                    "Adding",
                    format!("{:?} to `workspace.{}`", add, field),
                    termcolor::Color::Cyan,
                )?;
            }
        }
        if let Some(rm) = rm {
            let rm = relative_to_root(rm)?;
            let i = array.iter().position(|m| same_paths(m, rm));
            if let Some(i) = i {
                if !dry_run {
                    array.remove(i);
                }
                stderr.status_with_color(
                    "Removing",
                    format!("{:?} from `workspace.{}`", rm, field),
                    termcolor::Color::Red,
                )?;
            }
        }
    }

    let cargo_toml = cargo_toml.to_string();
    let modified = cargo_toml != orig;
    if modified {
        crate::fs::write(manifest_path, cargo_toml, dry_run)?;
    }
    Ok(modified)
}

trait WriteColorExt: WriteColor {
    fn warn(&mut self, message: impl Display) -> io::Result<()> {
        self.set_color(
            ColorSpec::new()
                .set_fg(Some(termcolor::Color::Yellow))
                .set_bold(true)
                .set_reset(false),
        )?;
        self.write_all(b"warning:")?;
        self.reset()?;
        writeln!(self, " {}", message)
    }

    fn status_with_color(
        &mut self,
        status: impl Display,
        message: impl Display,
        color: termcolor::Color,
    ) -> io::Result<()> {
        self.set_color(
            ColorSpec::new()
                .set_fg(Some(color))
                .set_bold(true)
                .set_reset(false),
        )?;
        write!(self, "{:>12}", status)?;
        self.reset()?;
        writeln!(self, " {}", message)
    }
}

impl<W: WriteColor> WriteColorExt for W {}
