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
    assert!(std::fs::read_dir("cpython-3.12.8")?.count() > 0);
    assert!(std::fs::read_dir("cpython-3.13.1")?.count() > 0);

    Ok(())
}

#[test]
fn download_package() -> anyhow::Result<()> {
    let temp_dir = tempdir()?;

    let assert = Command::cargo_bin("wasi-wheels")?
        .args([
            "download-package",
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

#[test]
fn build_pydantic() -> anyhow::Result<()> {
    let assert = Command::cargo_bin("wasi-wheels")?
        .args(["build", "pydantic-core", "2.27.2"])
        .assert();

    assert.success();

    assert!(std::fs::read_dir("packages/pydantic_core-2.27.2/dist")?.count() > 0);

    Ok(())
}
