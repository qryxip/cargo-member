#[doc(hidden)]
pub mod cli;
mod fs;

use anyhow::{anyhow, bail, ensure, Context as _};
use cargo_metadata::{Metadata, MetadataCommand};
use ignore::{Walk, WalkBuilder};
use itertools::Itertools as _;
use log::debug;
use std::{
    env,
    ffi::{OsStr, OsString},
    fmt::{self, Debug, Display},
    io::{self, Sink},
    iter,
    ops::Deref,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    slice, str, vec,
};
use termcolor::{ColorSpec, NoColor, WriteColor};

pub fn include<I: IntoIterator<Item = P>, P: AsRef<Path>>(
    workspace_root: &Path,
    paths: I,
) -> Include<NoColor<Sink>> {
    Include::new(workspace_root, paths)
}

pub fn exclude<I: IntoIterator<Item = P>, P: AsRef<Path>, W: WriteColor>(
    workspace_root: &Path,
    paths: I,
    stderr: W,
) -> Exclude<W> {
    Exclude::new(workspace_root, paths, stderr)
}

pub fn focus(workspace_root: &Path, path: &Path) -> Focus<NoColor<Sink>> {
    Focus::new(workspace_root, path)
}

pub fn new(workspace_root: &Path, path: &Path) -> New<NoColor<Sink>> {
    New::new(workspace_root, path)
}

pub fn cp(workspace_root: &Path, src: &Path, dst: &Path) -> Cp<NoColor<Sink>> {
    Cp::new(workspace_root, src, dst)
}

pub fn rm<I: IntoIterator<Item = P>, P: AsRef<Path>>(
    workspace_root: &Path,
    paths: I,
) -> Rm<NoColor<Sink>> {
    Rm::new(workspace_root, paths)
}

pub fn mv(workspace_root: &Path, src: &Path, dst: &Path) -> Mv<NoColor<Sink>> {
    Mv::new(workspace_root, src, dst)
}

#[derive(Debug)]
pub struct Include<W> {
    workspace_root: PathBuf,
    paths: Vec<PathBuf>,
    force: bool,
    dry_run: bool,
    stderr: W,
}

impl Include<NoColor<Sink>> {
    pub fn new<I: IntoIterator<Item = P>, P: AsRef<Path>>(workspace_root: &Path, paths: I) -> Self {
        Self {
            workspace_root: workspace_root.to_owned(),
            paths: paths.into_iter().map(|p| p.as_ref().to_owned()).collect(),
            force: false,
            dry_run: false,
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

    pub fn stderr<W2: WriteColor>(self, stderr: W2) -> Include<W2> {
        Include {
            workspace_root: self.workspace_root,
            paths: self.paths,
            force: self.force,
            dry_run: self.dry_run,
            stderr,
        }
    }

    pub fn exec(self) -> anyhow::Result<()> {
        let Self {
            workspace_root,
            paths,
            force,
            dry_run,
            mut stderr,
        } = self;

        for path in iter::once(&workspace_root).chain(&paths) {
            ensure_absolute(path)?;
        }

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
                &workspace_root,
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
                Some(&workspace_root.join("Cargo.toml")),
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
pub struct Exclude<W> {
    stderr: W,
    workspace_root: PathBuf,
    paths: Vec<PathBuf>,
    dry_run: bool,
}

impl<W: WriteColor> Exclude<W> {
    pub fn new<I: IntoIterator<Item = P>, P: AsRef<Path>>(
        workspace_root: &Path,
        paths: I,
        stderr: W,
    ) -> Self {
        Self {
            stderr,
            workspace_root: workspace_root.to_owned(),
            paths: paths.into_iter().map(|p| p.as_ref().to_owned()).collect(),
            dry_run: false,
        }
    }

    pub fn dry_run(self, dry_run: bool) -> Self {
        Self { dry_run, ..self }
    }

    pub fn exec(self) -> anyhow::Result<()> {
        let Self {
            mut stderr,
            workspace_root,
            paths,
            dry_run,
        } = self;

        for path in iter::once(&workspace_root).chain(&paths) {
            ensure_absolute(path)?;
        }

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
            cargo_metadata(
                Some(&workspace_root.join("Cargo.toml")),
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
    workspace_root: PathBuf,
    path: PathBuf,
    dry_run: bool,
    offline: bool,
    stderr: W,
}

impl Focus<NoColor<Sink>> {
    pub fn new(workspace_root: &Path, path: &Path) -> Self {
        Self {
            workspace_root: workspace_root.to_owned(),
            path: path.to_owned(),
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

        for path in &[&workspace_root, &path] {
            ensure_absolute(path)?;
        }

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
    workspace_root: PathBuf,
    path: PathBuf,
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
    pub fn new(workspace_root: &Path, path: &Path) -> Self {
        Self {
            workspace_root: workspace_root.to_owned(),
            path: path.to_owned(),
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
            workspace_root: self.workspace_root,
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
            workspace_root,
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

        include(&workspace_root, &[&path])
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
                .current_dir(&workspace_root)
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
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
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
    stderr: W,
    workspace_root: PathBuf,
    src: PathBuf,
    dst: PathBuf,
    dry_run: bool,
}

impl Cp<NoColor<Sink>> {
    pub fn new(workspace_root: &Path, src: &Path, dst: &Path) -> Self {
        Self {
            stderr: NoColor::new(io::sink()),
            workspace_root: workspace_root.to_owned(),
            src: src.to_owned(),
            dst: dst.to_owned(),
            dry_run: false,
        }
    }
}

impl<W: WriteColor> Cp<W> {
    pub fn stderr<W2: WriteColor>(self, stderr: W2) -> Cp<W2> {
        Cp {
            stderr,
            workspace_root: self.workspace_root,
            src: self.src,
            dst: self.dst,
            dry_run: self.dry_run,
        }
    }

    pub fn dry_run(self, dry_run: bool) -> Self {
        Self { dry_run, ..self }
    }

    pub fn exec(self) -> anyhow::Result<()> {
        let Self {
            mut stderr,
            workspace_root,
            src,
            dst,
            dry_run,
        } = self;

        for path in &[&workspace_root, &src, &dst] {
            ensure_absolute(path)?;
        }

        let dst = if dst.exists() {
            dst.join(src.file_name().expect("should be absolute"))
        } else {
            dst
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
        if dry_run {
            stderr.warn("not copying due to dry run")?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Rm<W> {
    stderr: W,
    workspace_root: PathBuf,
    paths: Vec<PathBuf>,
    force: bool,
    dry_run: bool,
}

impl Rm<NoColor<Sink>> {
    pub fn new<I: IntoIterator<Item = P>, P: AsRef<Path>>(workspace_root: &Path, paths: I) -> Self {
        Self {
            stderr: NoColor::new(io::sink()),
            workspace_root: workspace_root.to_owned(),
            paths: paths.into_iter().map(|p| p.as_ref().to_owned()).collect(),
            force: false,
            dry_run: false,
        }
    }
}

impl<W: WriteColor> Rm<W> {
    pub fn stderr<W2: WriteColor>(self, stderr: W2) -> Rm<W2> {
        Rm {
            stderr,
            workspace_root: self.workspace_root,
            paths: self.paths,
            force: self.force,
            dry_run: self.dry_run,
        }
    }

    pub fn force(self, force: bool) -> Self {
        Self { force, ..self }
    }

    pub fn dry_run(self, dry_run: bool) -> Self {
        Self { dry_run, ..self }
    }

    pub fn exec(self) -> anyhow::Result<()> {
        let Self {
            mut stderr,
            workspace_root,
            paths,
            force,
            dry_run,
        } = self;

        for path in iter::once(&workspace_root).chain(&paths) {
            ensure_absolute(path)?;
        }

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
            let result = cargo_metadata(
                Some(&workspace_root.join("Cargo.toml")),
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
    stderr: W,
    workspace_root: PathBuf,
    src: PathBuf,
    dst: PathBuf,
    dry_run: bool,
}

impl Mv<NoColor<Sink>> {
    pub fn new(workspace_root: &Path, src: &Path, dst: &Path) -> Self {
        Self {
            stderr: NoColor::new(io::sink()),
            workspace_root: workspace_root.to_owned(),
            src: src.to_owned(),
            dst: dst.to_owned(),
            dry_run: false,
        }
    }
}

impl<W: WriteColor> Mv<W> {
    pub fn stderr<W2: WriteColor>(self, stderr: W2) -> Mv<W2> {
        Mv {
            stderr,
            workspace_root: self.workspace_root,
            src: self.src,
            dst: self.dst,
            dry_run: self.dry_run,
        }
    }

    pub fn dry_run(self, dry_run: bool) -> Self {
        Self { dry_run, ..self }
    }

    pub fn exec(self) -> anyhow::Result<()> {
        let Self {
            mut stderr,
            workspace_root,
            src,
            dst,
            dry_run,
        } = self;

        cp(&workspace_root, &src, &dst)
            .dry_run(dry_run)
            .stderr(&mut stderr)
            .exec()?;
        rm(&workspace_root, &[src])
            .dry_run(dry_run)
            .stderr(stderr)
            .exec()
    }
}

fn ensure_absolute(path: &Path) -> anyhow::Result<()> {
    ensure!(path.is_absolute(), "must be absolute: {}", path.display());
    Ok(())
}

fn cargo_metadata(
    manifest_path: Option<&Path>,
    frozen: bool,
    locked: bool,
    offline: bool,
    cwd: &Path,
) -> cargo_metadata::Result<Metadata> {
    let mut cmd = MetadataCommand::new();
    if let Some(manifest_path) = manifest_path {
        cmd.manifest_path(manifest_path);
    }
    if frozen {
        cmd.other_options(&["--frozen".to_owned()]);
    }
    if offline {
        cmd.other_options(&["--offline".to_owned()]);
    }
    if locked {
        cmd.other_options(&["--locked".to_owned()]);
    }
    let metadata = cmd.current_dir(cwd).exec()?;
    debug!("workspace-root: {}", metadata.workspace_root.display());
    Ok(metadata)
}

fn modify_members<'a>(
    workspace_root: &Path,
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
        for add in *add {
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
