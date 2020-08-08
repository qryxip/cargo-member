#![warn(rust_2018_idioms)]

#[doc(hidden)]
pub mod cli;
mod fs;

use anyhow::{anyhow, bail, ensure, Context as _};
use cargo_metadata::{Metadata, MetadataCommand, Package, Resolve};
use easy_ext::ext;
use ignore::{Walk, WalkBuilder};
use itertools::Itertools as _;
use log::debug;
use serde::Deserialize;
use std::{
    env,
    ffi::{OsStr, OsString},
    fmt::{self, Debug, Display},
    io::{self, Sink},
    ops::Deref,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    slice, str, vec,
};
use termcolor::{ColorSpec, NoColor, WriteColor};
use url::Url;

#[derive(Debug)]
pub struct Include<W> {
    possibly_empty_workspace_root: anyhow::Result<PathBuf>,
    paths: anyhow::Result<Vec<PathBuf>>,
    force: bool,
    dry_run: bool,
    offline: bool,
    stderr: W,
}

impl Include<NoColor<Sink>> {
    pub fn new<Ps: IntoIterator<Item = P>, P: AsRef<Path>>(
        possibly_empty_workspace_root: &Path,
        paths: Ps,
    ) -> Self {
        Self {
            possibly_empty_workspace_root: ensure_absolute(possibly_empty_workspace_root),
            paths: paths.into_iter().map(ensure_absolute).collect(),
            force: false,
            dry_run: false,
            offline: false,
            stderr: NoColor::new(io::sink()),
        }
    }
}

impl<W: WriteColor> Include<W> {
    pub fn force(self, force: bool) -> Self {
        Self { force, ..self }
    }

    pub fn dry_run(self, dry_run: bool) -> Self {
        Self { dry_run, ..self }
    }

    pub fn offline(self, offline: bool) -> Self {
        Self { offline, ..self }
    }

    pub fn stderr<W2: WriteColor>(self, stderr: W2) -> Include<W2> {
        Include {
            possibly_empty_workspace_root: self.possibly_empty_workspace_root,
            paths: self.paths,
            force: self.force,
            dry_run: self.dry_run,
            offline: self.offline,
            stderr,
        }
    }

    pub fn exec(self) -> anyhow::Result<()> {
        let Self {
            possibly_empty_workspace_root,
            paths,
            force,
            dry_run,
            offline,
            mut stderr,
        } = self;

        let (possibly_empty_workspace_root, paths) = (possibly_empty_workspace_root?, paths?);

        let modified = paths.iter().try_fold(false, |acc, path| {
            if !(force || path.join("Cargo.toml").exists()) {
                return Err(
                    anyhow!("`{}` does not exist", path.join("Cargo.toml").display()).context(
                        format!(
                            "`{}` does not seem to be a package. enable `--force` to add",
                            path.display(),
                        ),
                    ),
                );
            }
            modify_members(
                &possibly_empty_workspace_root,
                &[path],
                &[],
                &[],
                &[path],
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
        } else {
            let result = cargo_metadata(
                Some(&possibly_empty_workspace_root.join("Cargo.toml")),
                false,
                false,
                offline,
                &possibly_empty_workspace_root,
            );

            if !force {
                result?;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Exclude<W> {
    workspace_root: anyhow::Result<PathBuf>,
    paths: anyhow::Result<Vec<PathBuf>>,
    dry_run: bool,
    stderr: W,
}

impl Exclude<NoColor<Sink>> {
    pub fn new<Ps: IntoIterator<Item = P>, P: AsRef<Path>>(
        workspace_root: &Path,
        paths: Ps,
    ) -> Self {
        Self {
            workspace_root: ensure_absolute(workspace_root),
            paths: paths.into_iter().map(ensure_absolute).collect(),
            dry_run: false,
            stderr: NoColor::new(io::sink()),
        }
    }

    pub fn from_metadata<
        Ps: IntoIterator<Item = P>,
        P: AsRef<Path>,
        Ss: IntoIterator<Item = S>,
        S: AsRef<str>,
    >(
        metadata: &Metadata,
        paths: Ps,
        specs: Ss,
    ) -> Self {
        Self {
            workspace_root: Ok(metadata.workspace_root.clone()),
            paths: paths
                .into_iter()
                .map(ensure_absolute)
                .chain(specs.into_iter().map(|spec| {
                    let member = metadata.query_for_member(Some(spec.as_ref()))?;
                    Ok(member
                        .manifest_path
                        .parent()
                        .expect(r#"`manifest_path` should end with "Cargo.toml""#)
                        .to_owned())
                }))
                .collect(),
            dry_run: false,
            stderr: NoColor::new(io::sink()),
        }
    }
}

impl<W: WriteColor> Exclude<W> {
    pub fn dry_run(self, dry_run: bool) -> Self {
        Self { dry_run, ..self }
    }

    pub fn stderr<W2: WriteColor>(self, stderr: W2) -> Exclude<W2> {
        Exclude {
            workspace_root: self.workspace_root,
            paths: self.paths,
            dry_run: self.dry_run,
            stderr,
        }
    }

    pub fn exec(self) -> anyhow::Result<()> {
        let Self {
            mut stderr,
            workspace_root,
            paths,
            dry_run,
        } = self;

        let (workspace_root, paths) = (workspace_root?, paths?);

        let modified = paths.iter().try_fold(false, |acc, path| {
            modify_members(
                &workspace_root,
                &[],
                &[path],
                &[path],
                &[],
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
        } else {
            cargo_metadata_unless_empty(
                &workspace_root.join("Cargo.toml"),
                false,
                false,
                false,
                &workspace_root,
            )?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Deactivate<W> {
    workspace_root: anyhow::Result<PathBuf>,
    paths: anyhow::Result<Vec<PathBuf>>,
    dry_run: bool,
    stderr: W,
}

impl Deactivate<NoColor<Sink>> {
    pub fn new<Ps: IntoIterator<Item = P>, P: AsRef<Path>>(
        workspace_root: &Path,
        paths: Ps,
    ) -> Self {
        Self {
            workspace_root: ensure_absolute(workspace_root),
            paths: paths.into_iter().map(ensure_absolute).collect(),
            dry_run: false,
            stderr: NoColor::new(io::sink()),
        }
    }

    pub fn from_metadata<
        Ps: IntoIterator<Item = P>,
        P: AsRef<Path>,
        Ss: IntoIterator<Item = S>,
        S: AsRef<str>,
    >(
        metadata: &Metadata,
        paths: Ps,
        specs: Ss,
    ) -> Self {
        Self {
            workspace_root: Ok(metadata.workspace_root.clone()),
            paths: paths
                .into_iter()
                .map(ensure_absolute)
                .chain(specs.into_iter().map(|spec| {
                    let member = metadata.query_for_member(Some(spec.as_ref()))?;
                    Ok(member
                        .manifest_path
                        .parent()
                        .expect(r#"`manifest_path` should end with "Cargo.toml""#)
                        .to_owned())
                }))
                .collect(),
            dry_run: false,
            stderr: NoColor::new(io::sink()),
        }
    }
}

impl<W: WriteColor> Deactivate<W> {
    pub fn dry_run(self, dry_run: bool) -> Self {
        Self { dry_run, ..self }
    }

    pub fn stderr<W2: WriteColor>(self, stderr: W2) -> Deactivate<W2> {
        Deactivate {
            workspace_root: self.workspace_root,
            paths: self.paths,
            dry_run: self.dry_run,
            stderr,
        }
    }

    pub fn exec(self) -> anyhow::Result<()> {
        let Self {
            mut stderr,
            workspace_root,
            paths,
            dry_run,
        } = self;

        let (workspace_root, paths) = (workspace_root?, paths?);

        let modified = paths.iter().try_fold(false, |acc, path| {
            modify_members(
                &workspace_root,
                &[],
                &[],
                &[path],
                &[path],
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
        } else {
            cargo_metadata_unless_empty(
                &workspace_root.join("Cargo.toml"),
                false,
                false,
                false,
                &workspace_root,
            )?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Focus<W> {
    workspace_root: anyhow::Result<PathBuf>,
    path: anyhow::Result<PathBuf>,
    dry_run: bool,
    offline: bool,
    stderr: W,
}

impl Focus<NoColor<Sink>> {
    pub fn new(workspace_root: &Path, path: &Path) -> Self {
        Self {
            workspace_root: ensure_absolute(workspace_root),
            path: ensure_absolute(path),
            dry_run: false,
            offline: false,
            stderr: NoColor::new(io::sink()),
        }
    }
}

impl<W: WriteColor> Focus<W> {
    pub fn dry_run(self, dry_run: bool) -> Self {
        Self { dry_run, ..self }
    }

    pub fn offline(self, offline: bool) -> Self {
        Self { offline, ..self }
    }

    pub fn stderr<W2: WriteColor>(self, stderr: W2) -> Focus<W2> {
        Focus {
            workspace_root: self.workspace_root,
            path: self.path,
            dry_run: self.dry_run,
            offline: self.offline,
            stderr,
        }
    }

    pub fn exec(self) -> anyhow::Result<()> {
        let Self {
            workspace_root,
            path,
            dry_run,
            offline,
            mut stderr,
        } = self;

        let (workspace_root, path) = (workspace_root?, path?);

        let mut exclude = vec![];
        for entry in Walk::new(&workspace_root) {
            match entry {
                Ok(entry) => {
                    if entry.path().ends_with("Cargo.toml") {
                        let dir = entry.path().parent().expect("should not empty");
                        if ![&*workspace_root, &*path].contains(&dir) {
                            exclude.push(dir.to_owned());
                        }
                    }
                }
                Err(err) => stderr.warn(err)?,
            }
        }
        let exclude = exclude.iter().map(Deref::deref).collect::<Vec<_>>();

        modify_members(
            &workspace_root,
            &[&path],
            &exclude,
            &exclude,
            &[&path],
            dry_run,
            &mut stderr,
        )?;

        if dry_run {
            stderr.warn("not modifying `workspace` due to dry run")?;
        } else {
            cargo_metadata(
                Some(&workspace_root.join("Cargo.toml")),
                false,
                false,
                offline,
                &workspace_root,
            )?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct New<W> {
    possibly_empty_workspace_root: anyhow::Result<PathBuf>,
    path: anyhow::Result<PathBuf>,
    cargo_new_registry: Option<String>,
    cargo_new_vcs: Option<String>,
    cargo_new_lib: bool,
    cargo_new_name: Option<String>,
    cargo_new_stderr_redirection: Stdio,
    offline: bool,
    dry_run: bool,
    stderr: W,
}

impl New<NoColor<Sink>> {
    pub fn new(possibly_empty_workspace_root: &Path, path: &Path) -> Self {
        Self {
            possibly_empty_workspace_root: ensure_absolute(possibly_empty_workspace_root),
            path: ensure_absolute(path),
            cargo_new_registry: None,
            cargo_new_vcs: None,
            cargo_new_lib: false,
            cargo_new_name: None,
            cargo_new_stderr_redirection: Stdio::null(),
            offline: false,
            dry_run: false,
            stderr: NoColor::new(io::sink()),
        }
    }
}

impl<W: WriteColor> New<W> {
    pub fn cargo_new_registry<S: AsRef<str>>(self, cargo_new_registry: Option<S>) -> Self {
        let cargo_new_registry = cargo_new_registry.map(|s| s.as_ref().to_owned());
        Self {
            cargo_new_registry,
            ..self
        }
    }

    pub fn cargo_new_vcs<S: AsRef<str>>(self, cargo_new_vcs: Option<S>) -> Self {
        let cargo_new_vcs = cargo_new_vcs.map(|s| s.as_ref().to_owned());
        Self {
            cargo_new_vcs,
            ..self
        }
    }

    pub fn cargo_new_lib(self, cargo_new_lib: bool) -> Self {
        Self {
            cargo_new_lib,
            ..self
        }
    }

    pub fn cargo_new_name<S: AsRef<str>>(self, cargo_new_name: Option<S>) -> Self {
        let cargo_new_name = cargo_new_name.map(|s| s.as_ref().to_owned());
        Self {
            cargo_new_name,
            ..self
        }
    }

    pub fn cargo_new_stderr_redirection(self, cargo_new_stderr_redirection: Stdio) -> Self {
        Self {
            cargo_new_stderr_redirection,
            ..self
        }
    }

    pub fn offline(self, offline: bool) -> Self {
        Self { offline, ..self }
    }

    pub fn dry_run(self, dry_run: bool) -> Self {
        Self { dry_run, ..self }
    }

    pub fn stderr<W2: WriteColor>(self, stderr: W2) -> New<W2> {
        New {
            possibly_empty_workspace_root: self.possibly_empty_workspace_root,
            path: self.path,
            cargo_new_registry: self.cargo_new_registry,
            cargo_new_vcs: self.cargo_new_vcs,
            cargo_new_lib: self.cargo_new_lib,
            cargo_new_name: self.cargo_new_name,
            cargo_new_stderr_redirection: self.cargo_new_stderr_redirection,
            offline: self.offline,
            dry_run: self.dry_run,
            stderr,
        }
    }

    pub fn exec(self) -> anyhow::Result<()> {
        let Self {
            possibly_empty_workspace_root,
            path,
            cargo_new_registry,
            cargo_new_vcs,
            cargo_new_lib,
            cargo_new_name,
            cargo_new_stderr_redirection,
            offline,
            dry_run,
            mut stderr,
        } = self;

        let (possibly_empty_workspace_root, path) = (possibly_empty_workspace_root?, path?);

        Include::new(&possibly_empty_workspace_root, &[&path])
            .force(true)
            .dry_run(dry_run)
            .stderr(&mut stderr)
            .exec()?;

        if dry_run {
            stderr.warn("not creating a new package due to dry run")?;
        } else {
            let cargo_exe = env::var_os("CARGO").with_context(|| "`$CARGO` should be present")?;

            let args = Args::new()
                .arg("new")
                .option(cargo_new_registry.as_ref(), "--registry")
                .option(cargo_new_vcs.as_ref(), "--vcs")
                .flag(cargo_new_lib, "--lib")
                .option(cargo_new_name.as_ref(), "--name")
                .flag(offline, "--offline")
                .arg(&path);

            let output = Command::new(&cargo_exe)
                .args(&args)
                .current_dir(&possibly_empty_workspace_root)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(cargo_new_stderr_redirection)
                .output()
                .with_context(|| format!("failed to execute `{}`", cargo_exe.to_string_lossy()))?;

            stderr.write_all(&output.stderr)?;

            if !output.status.success() {
                bail!(
                    "`{}{}` failed ({})",
                    shell_escape::escape(cargo_exe.to_string_lossy()),
                    args.0.iter().format_with("", |s, f| f(&format_args!(
                        " {}",
                        shell_escape::escape(s.to_string_lossy()),
                    ))),
                    output.status,
                );
            }

            cargo_metadata(None, false, false, offline, &possibly_empty_workspace_root)?;
        }
        Ok(())
    }
}

#[derive(Default, Debug)]
struct Args(Vec<OsString>);

impl Args {
    fn new() -> Self {
        Self::default()
    }

    fn flag(mut self, val: bool, long: &'static str) -> Self {
        if val {
            self.0.push(long.to_owned().into());
        }
        self
    }

    fn option(mut self, val: Option<impl AsRef<OsStr>>, long: &'static str) -> Self {
        if let Some(val) = val {
            self.0.push(long.to_owned().into());
            self.0.push(val.as_ref().to_owned());
        }
        self
    }

    fn arg(mut self, val: impl AsRef<OsStr>) -> Self {
        self.0.push(val.as_ref().to_owned());
        self
    }
}

impl AsRef<[OsString]> for Args {
    fn as_ref(&self) -> &[OsString] {
        &self.0
    }
}

impl Display for Args {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.0, fmt)
    }
}

impl<'a> IntoIterator for &'a Args {
    type Item = &'a OsString;
    type IntoIter = slice::Iter<'a, OsString>;

    fn into_iter(self) -> slice::Iter<'a, OsString> {
        self.0.iter()
    }
}

impl IntoIterator for Args {
    type Item = OsString;
    type IntoIter = vec::IntoIter<OsString>;

    fn into_iter(self) -> vec::IntoIter<OsString> {
        self.0.into_iter()
    }
}

#[derive(Debug)]
pub struct Cp<W> {
    src: anyhow::Result<PathBuf>,
    dst: anyhow::Result<PathBuf>,
    dry_run: bool,
    no_rename: bool,
    stderr: W,
}

impl Cp<NoColor<Sink>> {
    pub fn new(src: &Path, dst: &Path) -> Self {
        Self {
            src: ensure_absolute(src),
            dst: ensure_absolute(dst),
            dry_run: false,
            no_rename: false,
            stderr: NoColor::new(io::sink()),
        }
    }

    pub fn from_metadata(metadata: &Metadata, src: &str, dst: &Path) -> Self {
        Self {
            src: metadata.query_for_member(Some(src)).map(|member| {
                member
                    .manifest_path
                    .parent()
                    .expect(r#"`manifest_path` should end with "Cargo.toml""#)
                    .to_owned()
            }),
            dst: ensure_absolute(dst),
            dry_run: false,
            no_rename: false,
            stderr: NoColor::new(io::sink()),
        }
    }
}

impl<W: WriteColor> Cp<W> {
    pub fn dry_run(self, dry_run: bool) -> Self {
        Self { dry_run, ..self }
    }

    pub fn no_rename(self, no_rename: bool) -> Self {
        Self { no_rename, ..self }
    }

    pub fn stderr<W2: WriteColor>(self, stderr: W2) -> Cp<W2> {
        Cp {
            src: self.src,
            dst: self.dst,
            dry_run: self.dry_run,
            no_rename: self.no_rename,
            stderr,
        }
    }

    pub fn exec(self) -> anyhow::Result<()> {
        let Self {
            mut stderr,
            src,
            dst,
            dry_run,
            no_rename,
        } = self;

        let (src, dst) = (src?, dst?);

        let dst = if dst.exists() {
            dst.join(src.file_name().expect("should be absolute"))
        } else {
            dst
        };

        ensure!(!dst.exists(), "`{}` exists", dst.display());

        let mut cargo_toml = crate::fs::read_toml_edit(src.join("Cargo.toml"))
            .with_context(|| format!("`{}` does not seem to be a package", src.display()))?;
        if let Some(package) = cargo_toml["package"].as_table_mut() {
            package.remove("workspace");
            if !no_rename {
                let file_name = dst.file_name().expect("should exist");
                let file_name = file_name
                    .to_str()
                    .with_context(|| format!("{:?} is not valid UTF-8", file_name))?;
                package["name"] = toml_edit::value(file_name);
            }
        }

        stderr.status(
            "Copying",
            format!("`{}` to `{}`", src.display(), dst.display()),
        )?;

        let src_root = src;
        for src in WalkBuilder::new(&src_root).hidden(false).build() {
            match src {
                Ok(src) => {
                    let src = src.path();
                    if !(src.is_dir()
                        || src == src_root.join("Cargo.toml")
                        || src.starts_with(src_root.join(".git")))
                    {
                        let dst = dst.join(src.strip_prefix(&src_root)?);
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

        if let [_, dst_workspace_root] = &*dst
            .ancestors()
            .filter(|d| d.join("Cargo.toml").exists())
            .collect::<Vec<_>>()
        {
            stderr.status_with_color(
                "Found",
                format!("workspace at {}", dst_workspace_root.display()),
                termcolor::Color::Cyan,
            )?;

            modify_members(
                dst_workspace_root,
                &[&dst],
                &[],
                &[],
                &[&dst],
                dry_run,
                &mut stderr,
            )?;
        }

        if dry_run {
            stderr.warn("not copying due to dry run")?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Rm<W> {
    workspace_root: anyhow::Result<PathBuf>,
    paths: anyhow::Result<Vec<PathBuf>>,
    force: bool,
    dry_run: bool,
    stderr: W,
}

impl Rm<NoColor<Sink>> {
    pub fn new<Ps: IntoIterator<Item = P>, P: AsRef<Path>>(
        workspace_root: &Path,
        paths: Ps,
    ) -> Self {
        Self {
            workspace_root: ensure_absolute(workspace_root),
            paths: paths.into_iter().map(ensure_absolute).collect(),
            force: false,
            dry_run: false,
            stderr: NoColor::new(io::sink()),
        }
    }

    pub fn from_metadata<
        Ps: IntoIterator<Item = P>,
        P: AsRef<Path>,
        Ss: IntoIterator<Item = S>,
        S: AsRef<str>,
    >(
        metadata: &Metadata,
        paths: Ps,
        specs: Ss,
    ) -> Self {
        Self {
            workspace_root: Ok(metadata.workspace_root.clone()),
            paths: paths
                .into_iter()
                .map(ensure_absolute)
                .chain(specs.into_iter().map(|spec| {
                    let member = metadata.query_for_member(Some(spec.as_ref()))?;
                    Ok(member
                        .manifest_path
                        .parent()
                        .expect(r#"`manifest_path` should end with "Cargo.toml""#)
                        .to_owned())
                }))
                .collect(),
            force: false,
            dry_run: false,
            stderr: NoColor::new(io::sink()),
        }
    }
}

impl<W: WriteColor> Rm<W> {
    pub fn force(self, force: bool) -> Self {
        Self { force, ..self }
    }

    pub fn dry_run(self, dry_run: bool) -> Self {
        Self { dry_run, ..self }
    }

    pub fn stderr<W2: WriteColor>(self, stderr: W2) -> Rm<W2> {
        Rm {
            stderr,
            workspace_root: self.workspace_root,
            paths: self.paths,
            force: self.force,
            dry_run: self.dry_run,
        }
    }

    pub fn exec(self) -> anyhow::Result<()> {
        let Self {
            mut stderr,
            workspace_root,
            paths,
            force,
            dry_run,
        } = self;

        let (workspace_root, paths) = (workspace_root?, paths?);

        let modified = paths.iter().try_fold(false, |acc, path| {
            if !(force || path.join("Cargo.toml").exists()) {
                return Err(
                    anyhow!("`{}` does not exist", path.join("Cargo.toml").display()).context(
                        format!(
                            "`{}` does not seem to be a package. enable `--force` to remove",
                            path.display(),
                        ),
                    ),
                );
            }
            stderr.status_with_color(
                "Removing",
                format!("directory `{}`", path.display()),
                termcolor::Color::Red,
            )?;
            crate::fs::remove_dir_all(path, dry_run)?;
            modify_members(
                &workspace_root,
                &[],
                &[],
                &[path],
                &[path],
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
        } else {
            let result = cargo_metadata_unless_empty(
                &workspace_root.join("Cargo.toml"),
                false,
                false,
                false,
                &workspace_root,
            );

            if !force {
                result?;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Mv<W> {
    workspace_root: anyhow::Result<PathBuf>,
    src: anyhow::Result<PathBuf>,
    dst: anyhow::Result<PathBuf>,
    dry_run: bool,
    no_rename: bool,
    stderr: W,
}

impl Mv<NoColor<Sink>> {
    pub fn new(workspace_root: &Path, src: &Path, dst: &Path) -> Self {
        Self {
            workspace_root: ensure_absolute(workspace_root),
            src: ensure_absolute(src),
            dst: ensure_absolute(dst),
            dry_run: false,
            no_rename: false,
            stderr: NoColor::new(io::sink()),
        }
    }

    pub fn from_metadata(metadata: &Metadata, src: &str, dst: &Path) -> Self {
        Self {
            workspace_root: Ok(metadata.workspace_root.clone()),
            src: metadata.query_for_member(Some(src)).map(|member| {
                member
                    .manifest_path
                    .parent()
                    .expect(r#"`manifest_path` should end with "Cargo.toml""#)
                    .to_owned()
            }),
            dst: ensure_absolute(dst),
            dry_run: false,
            no_rename: false,
            stderr: NoColor::new(io::sink()),
        }
    }
}

impl<W: WriteColor> Mv<W> {
    pub fn dry_run(self, dry_run: bool) -> Self {
        Self { dry_run, ..self }
    }

    pub fn no_rename(self, no_rename: bool) -> Self {
        Self { no_rename, ..self }
    }

    pub fn stderr<W2: WriteColor>(self, stderr: W2) -> Mv<W2> {
        Mv {
            stderr,
            workspace_root: self.workspace_root,
            src: self.src,
            dst: self.dst,
            dry_run: self.dry_run,
            no_rename: self.no_rename,
        }
    }

    pub fn exec(self) -> anyhow::Result<()> {
        let Self {
            mut stderr,
            workspace_root,
            src,
            dst,
            dry_run,
            no_rename,
        } = self;

        let (workspace_root, src, dst) = (workspace_root?, src?, dst?);

        Cp::new(&src, &dst)
            .dry_run(dry_run)
            .no_rename(no_rename)
            .stderr(&mut stderr)
            .exec()?;

        Rm::new(&workspace_root, &[src])
            .dry_run(dry_run)
            .stderr(stderr)
            .exec()
    }
}

fn ensure_absolute(path: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
    let path = path.as_ref();
    ensure!(path.is_absolute(), "must be absolute: {}", path.display());
    Ok(path.to_owned())
}

fn cargo_metadata_unless_empty(
    manifest_path: &Path,
    frozen: bool,
    locked: bool,
    offline: bool,
    cwd: &Path,
) -> anyhow::Result<()> {
    let CargoToml { workspace, package } = crate::fs::read_toml(manifest_path)?;
    if !workspace.members.is_empty() || package.is_some() {
        cargo_metadata(Some(manifest_path), frozen, locked, offline, cwd)?;
    }
    return Ok(());

    #[derive(Deserialize)]
    struct CargoToml {
        #[serde(default)]
        workspace: CargoTomlWorkspace,
        package: Option<toml::Value>,
    }

    #[derive(Deserialize, Default)]
    struct CargoTomlWorkspace {
        members: Vec<String>,
    }
}

fn cargo_metadata(
    manifest_path: Option<&Path>,
    frozen: bool,
    locked: bool,
    offline: bool,
    cwd: &Path,
) -> anyhow::Result<Metadata> {
    let mut cmd = MetadataCommand::new();
    if let Some(manifest_path) = manifest_path {
        cmd.manifest_path(manifest_path);
    }
    if frozen {
        cmd.other_options(vec!["--frozen".to_owned()]);
    }
    if offline {
        cmd.other_options(vec!["--offline".to_owned()]);
    }
    if locked {
        cmd.other_options(vec!["--locked".to_owned()]);
    }
    let metadata = cmd.current_dir(cwd).exec().map_err(|err| match err {
        cargo_metadata::Error::CargoMetadata { stderr } => anyhow!("{}", stderr.trim_end()),
        err => err.into(),
    })?;
    debug!("workspace-root: {}", metadata.workspace_root.display());
    Ok(metadata)
}

fn modify_members<'a>(
    possibly_empty_workspace_root: &Path,
    add_to_workspace_members: &[&'a Path],
    add_to_workspace_exclude: &[&'a Path],
    rm_from_workspace_members: &[&'a Path],
    rm_from_workspace_exclude: &[&'a Path],
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
    .copied()
    .flatten()
    .any(|&p| p == possibly_empty_workspace_root)
    {
        bail!(
            "`{}` is the workspace root",
            possibly_empty_workspace_root.display()
        );
    }

    let manifest_path = possibly_empty_workspace_root.join("Cargo.toml");
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
            let path = path
                .strip_prefix(possibly_empty_workspace_root)
                .unwrap_or(path);
            path.to_str()
                .with_context(|| format!("{:?} is not valid UTF-8 path", path))
        };

        let same_paths = |value: &toml_edit::Value, target: &str| -> _ {
            value.as_str().map_or(false, |s| {
                possibly_empty_workspace_root.join(s) == possibly_empty_workspace_root.join(target)
            })
        };

        let array = cargo_toml["workspace"][field]
            .or_insert(toml_edit::value(toml_edit::Array::default()))
            .as_array_mut()
            .with_context(|| format!("`workspace.{}` must be an array", field))?;
        for add in *add {
            let add = relative_to_root(add)?;
            if array.iter().all(|m| !same_paths(m, add)) {
                if !dry_run {
                    array
                        .push(add)
                        .map_err(|_| anyhow!("`workspace.{}` must be an string array", field))?;
                }
                stderr.status("Adding", format!("{:?} to `workspace.{}`", add, field))?;
            }
        }
        for rm in *rm {
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
        writeln!(self, " {}", message)?;
        self.flush()
    }

    fn status(&mut self, status: impl Display, message: impl Display) -> io::Result<()> {
        self.status_with_color(status, message, termcolor::Color::Green)
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
        writeln!(self, " {}", message)?;
        self.flush()
    }
}

impl<W: WriteColor> WriteColorExt for W {}

#[ext(MetadataExt)]
impl Metadata {
    fn query_for_member<'a>(&'a self, spec: Option<&str>) -> anyhow::Result<&'a Package> {
        let cargo_exe = env::var_os("CARGO").with_context(|| "`$CARGO` should be present")?;

        let manifest_path = self
            .resolve
            .as_ref()
            .and_then(|Resolve { root, .. }| root.as_ref())
            .map(|id| self[id].manifest_path.clone())
            .unwrap_or_else(|| self.workspace_root.join("Cargo.toml"));

        let args = [
            Some("pkgid".as_ref()),
            Some("--manifest-path".as_ref()),
            Some(manifest_path.as_ref()),
            spec.map(OsStr::new),
        ];
        let args = args.iter().flatten();
        let output = duct::cmd(cargo_exe, args)
            .dir(&self.workspace_root)
            .stdout_capture()
            .stderr_capture()
            .unchecked()
            .run()?;
        let stdout = str::from_utf8(&output.stdout)?.trim_end();
        let stderr = str::from_utf8(&output.stderr)?.trim_end();
        if !output.status.success() {
            bail!("{}", stderr.trim_start_matches("error: "));
        }

        let url = stdout.parse::<Url>()?;
        let fragment = url.fragment().expect("the URL should contain fragment");
        let spec_name = match *fragment.splitn(2, ':').collect::<Vec<_>>() {
            [name, _] => name,
            [_] => url
                .path_segments()
                .and_then(Iterator::last)
                .expect("should contain name"),
            _ => unreachable!(),
        };

        self.packages
            .iter()
            .find(|Package { id, name, .. }| {
                self.workspace_members.contains(id) && name == spec_name
            })
            .with_context(|| {
                let spec = spec.expect("should be present here");
                format!("package `{}` is not a member of the workspace", spec)
            })
    }
}
