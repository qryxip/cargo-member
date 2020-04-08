use anyhow::{ensure, Context as _};
use cargo_metadata::{Metadata, Package, Resolve};
use easy_ext::ext;
use env_logger::fmt::WriteStyle;
use std::{
    env,
    ffi::OsStr,
    io::Write as _,
    path::{Path, PathBuf},
    process::{self, Stdio},
    str,
};
use structopt::{clap::AppSettings, StructOpt};
use strum::{EnumString, EnumVariantNames, IntoStaticStr, VariantNames as _};
use termcolor::{BufferedStandardStream, ColorSpec, WriteColor};
use url::Url;

#[derive(StructOpt, Debug)]
#[structopt(
    author,
    about,
    bin_name("cargo"),
    global_settings(&[AppSettings::DeriveDisplayOrder, AppSettings::UnifiedHelpMessage])
)]
pub enum Cargo {
    #[structopt(author, about)]
    Member(CargoMember),
}

#[derive(StructOpt, Debug)]
pub enum CargoMember {
    /// Include a member in the workspace
    Include(CargoMemberInclude),

    /// Exclude a member from the workspace
    Exclude(CargoMemberExclude),

    /// Include a package excluding the others
    Focus(CargoMemberFocus),

    /// Create a new package with `cargo new`
    New(CargoMemberNew),

    /// Copy a member in the workspace
    Cp(CargoMemberCp),

    /// Remove a member from the workspace
    Rm(CargoMemberRm),

    /// Move a member in the workspace
    Mv(CargoMemberMv),
}

impl CargoMember {
    pub fn color(&self) -> self::ColorChoice {
        match *self {
            Self::Include(CargoMemberInclude { color, .. })
            | Self::Exclude(CargoMemberExclude { color, .. })
            | Self::Focus(CargoMemberFocus { color, .. })
            | Self::New(CargoMemberNew { color, .. })
            | Self::Cp(CargoMemberCp { color, .. })
            | Self::Rm(CargoMemberRm { color, .. })
            | Self::Mv(CargoMemberMv { color, .. }) => color,
        }
    }
}

#[derive(StructOpt, Debug)]
pub struct CargoMemberInclude {
    /// [cargo] Path to Cargo.toml
    #[structopt(long, value_name("PATH"))]
    pub manifest_path: Option<PathBuf>,

    /// [cargo] Coloring
    #[structopt(
        long,
        value_name("WHEN"),
        possible_values(self::ColorChoice::VARIANTS),
        default_value("auto")
    )]
    pub color: self::ColorChoice,

    /// [cargo] Run without accessing the network
    #[structopt(long)]
    pub offline: bool,

    /// Allow non package paths
    #[structopt(long)]
    pub force: bool,

    /// Dry run. Also enables `--frozen` and `--locked`
    #[structopt(long)]
    pub dry_run: bool,

    /// Paths to include
    pub paths: Vec<PathBuf>,
}

#[derive(StructOpt, Debug)]
pub struct CargoMemberExclude {
    /// [cargo] Package(s) to exclude
    #[structopt(short, long, value_name("SPEC"), min_values(1), number_of_values(1))]
    pub package: Vec<String>,

    /// [cargo] Path to Cargo.toml
    #[structopt(long, value_name("PATH"))]
    pub manifest_path: Option<PathBuf>,

    /// [cargo] Coloring
    #[structopt(
        long,
        value_name("WHEN"),
        possible_values(self::ColorChoice::VARIANTS),
        default_value("auto")
    )]
    pub color: self::ColorChoice,

    /// [cargo] Run without accessing the network
    #[structopt(long)]
    pub offline: bool,

    /// Dry run. Also enables `--frozen` and `--locked`
    #[structopt(long)]
    pub dry_run: bool,

    /// Paths to exclude
    pub paths: Vec<PathBuf>,
}

#[derive(StructOpt, Debug)]
pub struct CargoMemberFocus {
    /// Dry run. Also enables `--frozen` and `--locked`
    #[structopt(long)]
    pub dry_run: bool,

    /// [cargo] Path to Cargo.toml
    #[structopt(long, value_name("PATH"))]
    pub manifest_path: Option<PathBuf>,

    /// [cargo] Coloring
    #[structopt(
        long,
        value_name("WHEN"),
        possible_values(self::ColorChoice::VARIANTS),
        default_value("auto")
    )]
    pub color: self::ColorChoice,

    /// [cargo] Run without accessing the network
    #[structopt(long)]
    pub offline: bool,

    /// Path to focus
    pub path: PathBuf,
}

#[derive(StructOpt, Debug)]
pub struct CargoMemberNew {
    /// [cargo] Path to Cargo.toml
    #[structopt(long, value_name("PATH"))]
    pub manifest_path: Option<PathBuf>,

    /// [cargo-new] Registry to use
    #[structopt(long, value_name("REGISTRY"))]
    pub registry: Option<String>,

    /// [cargo-new] Initialize a new repository for the given version control system (git, hg, pijul, or fossil) or do not initialize any version control at all (none), overriding a global configuration.
    #[structopt(
        long,
        value_name("VCS"),
        possible_values(&["git", "hg", "pijul", "fossil", "none"])
    )]
    pub vcs: Option<String>,

    /// [cargo-new] Use a library template
    #[structopt(long)]
    pub lib: bool,

    /// [cargo-new] Set the resulting package name, defaults to the directory name
    #[structopt(long, value_name("NAME"))]
    pub name: Option<String>,

    /// [cargo] Coloring
    #[structopt(
        long,
        value_name("WHEN"),
        possible_values(self::ColorChoice::VARIANTS),
        default_value("auto")
    )]
    pub color: self::ColorChoice,

    /// [cargo] Run without accessing the network
    #[structopt(long)]
    pub offline: bool,

    /// Dry run. Also enables `--frozen` and `--locked`
    #[structopt(long)]
    pub dry_run: bool,

    /// [cargo-new] Path
    pub path: PathBuf,
}

#[derive(StructOpt, Debug)]
pub struct CargoMemberCp {
    /// [cargo] Path to Cargo.toml
    #[structopt(long, value_name("PATH"))]
    pub manifest_path: Option<PathBuf>,

    /// [cargo] Coloring
    #[structopt(
        long,
        value_name("WHEN"),
        possible_values(self::ColorChoice::VARIANTS),
        default_value("auto")
    )]
    pub color: self::ColorChoice,

    /// [cargo] Run without accessing the network
    #[structopt(long)]
    pub offline: bool,

    /// Dry run. Also enables `--frozen` and `--locked`
    #[structopt(long)]
    pub dry_run: bool,

    /// Package ID specification
    pub src: String,

    /// Directory
    pub dst: PathBuf,
}

#[derive(StructOpt, Debug)]
pub struct CargoMemberRm {
    /// [cargo] Package(s) to exclude
    #[structopt(short, long, value_name("SPEC"), min_values(1), number_of_values(1))]
    pub package: Vec<String>,

    /// [cargo] Path to Cargo.toml
    #[structopt(long, value_name("PATH"))]
    pub manifest_path: Option<PathBuf>,

    /// [cargo] Coloring
    #[structopt(
        long,
        value_name("WHEN"),
        possible_values(self::ColorChoice::VARIANTS),
        default_value("auto")
    )]
    pub color: self::ColorChoice,

    /// [cargo] Run without accessing the network
    #[structopt(long)]
    pub offline: bool,

    /// Allow non package paths
    #[structopt(long)]
    pub force: bool,

    /// Dry run. Also enables `--frozen` and `--locked`
    #[structopt(long)]
    pub dry_run: bool,

    /// Paths to exclude
    pub paths: Vec<PathBuf>,
}

#[derive(StructOpt, Debug)]
pub struct CargoMemberMv {
    /// [cargo] Path to Cargo.toml
    #[structopt(long, value_name("PATH"))]
    pub manifest_path: Option<PathBuf>,

    /// [cargo] Coloring
    #[structopt(
        long,
        value_name("WHEN"),
        possible_values(self::ColorChoice::VARIANTS),
        default_value("auto")
    )]
    pub color: self::ColorChoice,

    /// [cargo] Run without accessing the network
    #[structopt(long)]
    pub offline: bool,

    /// Dry run. Also enables `--frozen` and `--locked`
    #[structopt(long)]
    pub dry_run: bool,

    /// Package ID specification
    pub src: String,

    /// Directory
    pub dst: PathBuf,
}

/// Coloring.
#[derive(EnumString, EnumVariantNames, IntoStaticStr, Clone, Copy, Debug)]
#[strum(serialize_all = "kebab-case")]
pub enum ColorChoice {
    Auto,
    Always,
    Never,
}

impl From<self::ColorChoice> for WriteStyle {
    fn from(choice: self::ColorChoice) -> Self {
        match choice {
            self::ColorChoice::Auto => Self::Auto,
            self::ColorChoice::Always => Self::Always,
            self::ColorChoice::Never => Self::Never,
        }
    }
}

#[derive(Debug)]
pub struct Context<W> {
    cwd: PathBuf,
    stderr: W,
    stderr_redirection: Stdio,
}

impl<W> Context<W> {
    pub fn new(stderr: W) -> anyhow::Result<Self> {
        let cwd = env::current_dir().with_context(|| "failed to get CWD")?;
        let stderr_redirection = Stdio::inherit();
        Ok(Self {
            cwd,
            stderr,
            stderr_redirection,
        })
    }
}

pub fn init_logger(color: self::ColorChoice) {
    env_logger::Builder::from_default_env()
        .write_style(color.into())
        .init();
}

pub fn stderr(color: self::ColorChoice) -> BufferedStandardStream {
    BufferedStandardStream::stderr(match color {
        self::ColorChoice::Auto if atty::is(atty::Stream::Stderr) => termcolor::ColorChoice::Auto,
        self::ColorChoice::Always => termcolor::ColorChoice::Always,
        self::ColorChoice::Auto | self::ColorChoice::Never => termcolor::ColorChoice::Never,
    })
}

pub fn exit_with_error(error: anyhow::Error, color: self::ColorChoice) -> ! {
    let mut stderr = BufferedStandardStream::stderr(match color {
        self::ColorChoice::Auto if atty::is(atty::Stream::Stderr) => termcolor::ColorChoice::Auto,
        self::ColorChoice::Always => termcolor::ColorChoice::Always,
        self::ColorChoice::Auto | self::ColorChoice::Never => termcolor::ColorChoice::Never,
    });

    let _ = stderr.set_color(
        ColorSpec::new()
            .set_fg(Some(termcolor::Color::Red))
            .set_bold(true)
            .set_reset(false),
    );
    let _ = stderr.write_all(b"error: ");
    let _ = stderr.reset();
    let _ = writeln!(stderr, "{}", error);

    for error in error.chain().skip(1) {
        let _ = writeln!(stderr, "\nCaused by:\n  {}", error);
    }

    let _ = stderr.flush();
    process::exit(101);
}

pub fn run(opt: CargoMember, ctx: Context<impl WriteColor>) -> anyhow::Result<()> {
    match opt {
        CargoMember::Include(opt) => include(opt, ctx),
        CargoMember::Exclude(opt) => exclude(opt, ctx),
        CargoMember::Focus(opt) => focus(opt, ctx),
        CargoMember::New(opt) => new(opt, ctx),
        CargoMember::Cp(opt) => cp(opt, ctx),
        CargoMember::Rm(opt) => rm(opt, ctx),
        CargoMember::Mv(opt) => mv(opt, ctx),
    }
}

fn include(opt: CargoMemberInclude, ctx: Context<impl WriteColor>) -> anyhow::Result<()> {
    let CargoMemberInclude {
        manifest_path,
        offline,
        force,
        dry_run,
        paths,
        ..
    } = opt;

    let Context { cwd, stderr, .. } = ctx;

    let Metadata { workspace_root, .. } =
        crate::cargo_metadata(manifest_path.as_deref(), dry_run, dry_run, offline, &cwd)?;
    let paths = paths.into_iter().map(|p| cwd.join(p.trim_leading_dots()));
    crate::include(&workspace_root, paths)
        .force(force)
        .dry_run(dry_run)
        .stderr(stderr)
        .exec()
}

fn exclude(opt: CargoMemberExclude, ctx: Context<impl WriteColor>) -> anyhow::Result<()> {
    let CargoMemberExclude {
        package,
        manifest_path,
        offline,
        dry_run,
        paths,
        ..
    } = opt;

    let Context { cwd, stderr, .. } = ctx;

    let metadata =
        crate::cargo_metadata(manifest_path.as_deref(), dry_run, dry_run, offline, &cwd)?;
    let paths = paths
        .into_iter()
        .map(|p| Ok(cwd.join(p.trim_leading_dots())))
        .chain(package.into_iter().map(|spec| {
            let member = metadata.query_for_member(Some(&spec))?;
            Ok(member
                .manifest_path
                .parent()
                .expect(r#"`manifest_path` should end with "Cargo.toml""#)
                .to_owned())
        }))
        .collect::<anyhow::Result<Vec<_>>>()?;

    crate::exclude(&metadata.workspace_root, paths, stderr)
        .dry_run(dry_run)
        .exec()
}

fn focus(opt: CargoMemberFocus, ctx: Context<impl WriteColor>) -> anyhow::Result<()> {
    let CargoMemberFocus {
        dry_run,
        manifest_path,
        offline,
        path,
        ..
    } = opt;

    let Context { cwd, stderr, .. } = ctx;

    let Metadata { workspace_root, .. } =
        crate::cargo_metadata(manifest_path.as_deref(), dry_run, dry_run, offline, &cwd)?;
    let path = cwd.join(path.trim_leading_dots());

    crate::focus(&workspace_root, &path)
        .dry_run(dry_run)
        .offline(offline)
        .stderr(stderr)
        .exec()
}

fn new(opt: CargoMemberNew, ctx: Context<impl WriteColor>) -> anyhow::Result<()> {
    let CargoMemberNew {
        manifest_path,
        registry,
        vcs,
        lib,
        name,
        offline,
        dry_run,
        path,
        ..
    } = opt;

    let Context {
        cwd,
        stderr,
        stderr_redirection,
    } = ctx;

    let Metadata { workspace_root, .. } =
        crate::cargo_metadata(manifest_path.as_deref(), dry_run, dry_run, offline, &cwd)?;
    let path = cwd.join(path.trim_leading_dots());

    crate::new(&workspace_root, &path)
        .cargo_new_registry(registry)
        .cargo_new_vcs(vcs)
        .cargo_new_lib(lib)
        .cargo_new_name(name)
        .cargo_new_stderr_redirection(stderr_redirection)
        .offline(offline)
        .dry_run(dry_run)
        .stderr(stderr)
        .exec()
}

fn cp(opt: CargoMemberCp, ctx: Context<impl WriteColor>) -> anyhow::Result<()> {
    let CargoMemberCp {
        manifest_path,
        offline,
        dry_run,
        src,
        dst,
        ..
    } = opt;

    let Context { cwd, stderr, .. } = ctx;

    let metadata =
        crate::cargo_metadata(manifest_path.as_deref(), dry_run, dry_run, offline, &cwd)?;
    let src = metadata
        .query_for_member(Some(&src))?
        .manifest_path
        .parent()
        .expect(r#"`manifest_path` should end with "Cargo.toml""#);
    let dst = cwd.join(dst.trim_leading_dots());

    crate::cp(&metadata.workspace_root, &src, &dst)
        .dry_run(dry_run)
        .stderr(stderr)
        .exec()
}

fn rm(opt: CargoMemberRm, ctx: Context<impl WriteColor>) -> anyhow::Result<()> {
    let CargoMemberRm {
        package,
        manifest_path,
        offline,
        force,
        dry_run,
        paths,
        ..
    } = opt;

    let Context { cwd, stderr, .. } = ctx;

    let metadata =
        crate::cargo_metadata(manifest_path.as_deref(), dry_run, dry_run, offline, &cwd)?;
    let paths = paths
        .into_iter()
        .map(|p| Ok(cwd.join(p.trim_leading_dots())))
        .chain(package.into_iter().map(|spec| {
            let member = metadata.query_for_member(Some(&spec))?;
            Ok(member
                .manifest_path
                .parent()
                .expect(r#"`manifest_path` should end with "Cargo.toml""#)
                .to_owned())
        }))
        .collect::<anyhow::Result<Vec<_>>>()?;

    crate::rm(&metadata.workspace_root, &paths)
        .force(force)
        .dry_run(dry_run)
        .stderr(stderr)
        .exec()
}

fn mv(opt: CargoMemberMv, ctx: Context<impl WriteColor>) -> anyhow::Result<()> {
    let CargoMemberMv {
        manifest_path,
        offline,
        dry_run,
        src,
        dst,
        ..
    } = opt;

    let Context { cwd, stderr, .. } = ctx;

    let metadata =
        crate::cargo_metadata(manifest_path.as_deref(), dry_run, dry_run, offline, &cwd)?;
    let src = metadata
        .query_for_member(Some(&src))?
        .manifest_path
        .parent()
        .expect(r#"`manifest_path` should end with "Cargo.toml""#);
    let dst = cwd.join(dst.trim_leading_dots());

    crate::mv(&metadata.workspace_root, &src, &dst)
        .dry_run(dry_run)
        .stderr(stderr)
        .exec()
}

#[ext(PathExt)]
impl Path {
    fn trim_leading_dots(&self) -> &Self {
        let mut acc = self;
        while let Ok(removed) = acc.strip_prefix(".") {
            acc = removed;
        }
        acc
    }
}

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
        ensure!(output.status.success(), "{}", stderr);

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
