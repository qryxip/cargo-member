use cargo_metadata::MetadataCommand;
use difference::assert_diff;
use duct::cmd;
use std::{
    env, fs, io,
    path::Path,
    str::{self, Utf8Error},
};
use tempdir::TempDir;
use termcolor::NoColor;

#[test]
fn normal() -> anyhow::Result<()> {
    let tempdir = TempDir::new("cargo-member-test-exclude-normal")?;

    fs::write(tempdir.path().join("Cargo.toml"), ORIGINAL)?;
    cargo_new(&tempdir.path().join("a"))?;
    cargo_new(&tempdir.path().join("b"))?;

    let mut stderr = vec![];

    cargo_member::exclude(tempdir.path(), &[tempdir.path().join("b")])
        .dry_run(false)
        .stderr(NoColor::new(&mut stderr))
        .exec()?;

    assert_manifest(&tempdir.path().join("Cargo.toml"), EXPECTED_MANIFEST)?;
    assert_stderr(&stderr, EXPECTED_STDERR)?;
    cargo_metadata(&tempdir.path().join("Cargo.toml"), &["--locked"])?;
    return Ok(());

    static ORIGINAL: &str = r#"[workspace]
members = ["a", "b"]
exclude = []
"#;

    static EXPECTED_MANIFEST: &str = r#"[workspace]
members = ["a"]
exclude = ["b"]
"#;

    static EXPECTED_STDERR: &str = r#"    Removing "b" from `workspace.members`
      Adding "b" to `workspace.exclude`
"#;
}

#[test]
fn dry_run() -> anyhow::Result<()> {
    let tempdir = TempDir::new("cargo-member-test-exclude-dry-run")?;

    fs::write(tempdir.path().join("Cargo.toml"), MANIFEST)?;
    cargo_new(&tempdir.path().join("a"))?;
    cargo_new(&tempdir.path().join("b"))?;
    cargo_metadata(&tempdir.path().join("Cargo.toml"), &[])?;

    let mut stderr = vec![];

    cargo_member::exclude(tempdir.path(), &[tempdir.path().join("b")])
        .dry_run(true)
        .stderr(NoColor::new(&mut stderr))
        .exec()?;

    assert_manifest(&tempdir.path().join("Cargo.toml"), MANIFEST)?;
    assert_stderr(&stderr, EXPECTED_STDERR)?;
    cargo_metadata(&tempdir.path().join("Cargo.toml"), &["--locked"])?;
    return Ok(());

    static MANIFEST: &str = r#"[workspace]
members = ["a", "b"]
exclude = []
"#;

    static EXPECTED_STDERR: &str = r#"    Removing "b" from `workspace.members`
      Adding "b" to `workspace.exclude`
warning: `workspace` unchanged
warning: not modifying the manifest due to dry run
"#;
}

fn cargo_new(path: &Path) -> io::Result<()> {
    let cargo_exe = env::var("CARGO").unwrap();
    cmd!(cargo_exe, "new", "-q", "--vcs", "none", path).run()?;
    Ok(())
}

fn assert_manifest(manifest_path: &Path, expected: &str) -> io::Result<()> {
    let modified = fs::read_to_string(manifest_path)?;
    assert_diff!(expected, &modified, "\n", 0);
    Ok(())
}

fn assert_stderr(stderr: &[u8], expected: &str) -> std::result::Result<(), Utf8Error> {
    assert_diff!(expected, str::from_utf8(stderr)?, "\n", 0);
    Ok(())
}

fn cargo_metadata(manifest_path: &Path, opts: &[&str]) -> cargo_metadata::Result<()> {
    let opts = opts
        .iter()
        .copied()
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    MetadataCommand::new()
        .manifest_path(manifest_path)
        .other_options(&opts)
        .exec()
        .map(drop)
}
