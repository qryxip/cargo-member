#![warn(rust_2018_idioms)]

use cargo_metadata::MetadataCommand;
use difference::assert_diff;
use std::{
    fs, io,
    path::Path,
    process::Stdio,
    str::{self, Utf8Error},
};
use tempdir::TempDir;
use termcolor::NoColor;

#[test]
fn new() -> anyhow::Result<()> {
    let tempdir = TempDir::new("cargo-member-new")?;

    fs::write(tempdir.path().join("Cargo.toml"), ORIGINAL)?;
    cargo_metadata(&tempdir.path().join("Cargo.toml"), &[]).unwrap_err();

    let mut stderr = vec![];

    cargo_member::new(tempdir.path(), &tempdir.path().join("a"))
        .cargo_new_registry(None::<&str>)
        .cargo_new_vcs(None::<&str>)
        .cargo_new_lib(false)
        .cargo_new_name(None::<&str>)
        .cargo_new_stderr_redirection(Stdio::null())
        .offline(true)
        .dry_run(false)
        .stderr(NoColor::new(&mut stderr))
        .exec()?;

    assert_manifest(&tempdir.path().join("Cargo.toml"), EXPECTED_MANIFEST)?;
    assert_stderr(&stderr, EXPECTED_STDERR)?;
    cargo_metadata(&tempdir.path().join("Cargo.toml"), &["--locked"])?;
    return Ok(());

    static ORIGINAL: &str = r#"[workspace]
members = []
exclude = []
"#;

    static EXPECTED_MANIFEST: &str = r#"[workspace]
members = ["a"]
exclude = []
"#;

    static EXPECTED_STDERR: &str = r#"      Adding "a" to `workspace.members`
"#;
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
