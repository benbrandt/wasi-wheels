//! Integration tests for the main CLI.

use assert_cmd::Command;
use tempfile::tempdir;

#[test]
fn install_build_tools() -> anyhow::Result<()> {
    let assert = Command::cargo_bin("wasi-wheels")?
        .arg("install-build-tools")
        .assert();

    assert.success();

    assert!(std::fs::read_dir("wasi-sdk")?.count() > 0);
    assert!(std::fs::read_dir("cpython")?.count() > 0);

    Ok(())
}

#[test]
fn download_sdist() -> anyhow::Result<()> {
    let temp_dir = tempdir()?;

    let assert = Command::cargo_bin("wasi-wheels")?
        .args([
            "sdist",
            "pydantic-core",
            "2.27.2",
            "-o",
            temp_dir.path().to_str().unwrap(),
        ])
        .assert();

    assert.success();

    assert!(std::fs::read_dir(temp_dir.path().join("pydantic_core-2.27.2"))?.count() > 0);

    Ok(())
}
