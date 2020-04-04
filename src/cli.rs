use anyhow::{ensure, Context as _};
use cargo_metadata::{Metadata, MetadataCommand, Package, Resolve};
use easy_ext::ext;
use env_logger::fmt::WriteStyle;
use log::debug;
use std::{
    env,
    ffi::OsStr,
    io::Write as _,
    path::{Path, PathBuf},
    process, str,
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

    /// Allow non package paths
    #[structopt(long)]
    pub force: bool,

    /// Dry run
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

    /// Dry run
    #[structopt(long)]
    pub dry_run: bool,

    /// Paths to exclude
    pub paths: Vec<PathBuf>,
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

    /// Dry run
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

    /// Allow non package paths
    #[structopt(long)]
    pub force: bool,

    /// Dry run
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

    /// Dry run
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
}

impl<W> Context<W> {
    pub fn new(stderr: W) -> anyhow::Result<Self> {
        let cwd = env::current_dir().with_context(|| "failed to get CWD")?;
        Ok(Self { cwd, stderr })
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
        CargoMember::Cp(opt) => cp(opt, ctx),
        CargoMember::Rm(opt) => rm(opt, ctx),
        CargoMember::Mv(opt) => mv(opt, ctx),
    }
}

fn include(opt: CargoMemberInclude, ctx: Context<impl WriteColor>) -> anyhow::Result<()> {
    let CargoMemberInclude {
        manifest_path,
        force,
        dry_run,
        paths,
        ..
    } = opt;

    let Context {
        cwd, mut stderr, ..
    } = ctx;

    let Metadata { workspace_root, .. } = cargo_metadata(manifest_path.as_deref(), &cwd)?;
    let paths = paths
        .into_iter()
        .map(|p| cwd.join(p.trim_leading_dots()))
        .collect::<Vec<_>>();
    crate::include(&workspace_root, &paths, force, dry_run, &mut stderr)
}

fn exclude(opt: CargoMemberExclude, ctx: Context<impl WriteColor>) -> anyhow::Result<()> {
    let CargoMemberExclude {
        package,
        manifest_path,
        dry_run,
        paths,
        ..
    } = opt;

    let Context {
        cwd, mut stderr, ..
    } = ctx;

    let metadata = cargo_metadata(manifest_path.as_deref(), &cwd)?;
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

    crate::exclude(&metadata.workspace_root, &paths, dry_run, &mut stderr)
}

fn cp(opt: CargoMemberCp, ctx: Context<impl WriteColor>) -> anyhow::Result<()> {
    let CargoMemberCp {
        manifest_path,
        dry_run,
        src,
        dst,
        ..
    } = opt;

    let Context {
        cwd, mut stderr, ..
    } = ctx;

    let metadata = cargo_metadata(manifest_path.as_deref(), &cwd)?;
    let src = metadata
        .query_for_member(Some(&src))?
        .manifest_path
        .parent()
        .expect(r#"`manifest_path` should end with "Cargo.toml""#);
    let dst = cwd.join(dst.trim_leading_dots());

    crate::cp(&metadata.workspace_root, &src, &dst, dry_run, &mut stderr)
}

fn rm(opt: CargoMemberRm, ctx: Context<impl WriteColor>) -> anyhow::Result<()> {
    let CargoMemberRm {
        package,
        manifest_path,
        force,
        dry_run,
        paths,
        ..
    } = opt;

    let Context {
        cwd, mut stderr, ..
    } = ctx;

    let metadata = cargo_metadata(manifest_path.as_deref(), &cwd)?;
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

    crate::rm(
        &metadata.workspace_root,
        &paths,
        force,
        dry_run,
        &mut stderr,
    )
}

fn mv(opt: CargoMemberMv, ctx: Context<impl WriteColor>) -> anyhow::Result<()> {
    let CargoMemberMv {
        manifest_path,
        dry_run,
        src,
        dst,
        ..
    } = opt;

    let Context {
        cwd, mut stderr, ..
    } = ctx;

    let metadata = cargo_metadata(manifest_path.as_deref(), &cwd)?;
    let src = metadata
        .query_for_member(Some(&src))?
        .manifest_path
        .parent()
        .expect(r#"`manifest_path` should end with "Cargo.toml""#);
    let dst = cwd.join(dst.trim_leading_dots());

    crate::mv(&metadata.workspace_root, &src, &dst, dry_run, &mut stderr)
}

fn cargo_metadata(manifest_path: Option<&Path>, cwd: &Path) -> cargo_metadata::Result<Metadata> {
    let mut cmd = MetadataCommand::new();
    if let Some(manifest_path) = manifest_path {
        cmd.manifest_path(manifest_path);
    }
    let metadata = cmd.current_dir(cwd).exec()?;
    debug!("workspace-root: {}", metadata.workspace_root.display());
    Ok(metadata)
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
