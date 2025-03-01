//! Integration tests for the main CLI.

use std::path::Path;

use assert_cmd::Command;
use tempfile::tempdir;

#[test]
fn install_build_tools() -> anyhow::Result<()> {
    let assert = Command::cargo_bin("wasi-wheels")?
        .arg("install-build-tools")
        .assert();

    assert.success();

    assert!(std::fs::read_dir("wasi-sdk")?.count() > 0);
    assert!(std::fs::read_dir("cpython-3.12.9")?.count() > 0);
    assert!(std::fs::read_dir("cpython-3.13.2")?.count() > 0);

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

#[test]
fn python_component() {
    let path = Path::new("./tests/test-component");
    Command::new("python3")
        .args(["-m", "venv", ".venv"])
        .current_dir(path)
        .assert()
        .success();

    Command::new("./.venv/bin/pip")
        .args(["install", ".[test]"])
        .current_dir(path)
        .assert()
        .success();

    Command::new("./.venv/bin/pip")
        .args([
            "install",
            "--target",
            "wasi_deps",
            "--platform",
            "any",
            "--platform",
            "wasi_0_0_0_wasm32",
            "--python-version",
            "3.12",
            "--only-binary",
            ":all:",
            "--index-url",
            "https://benbrandt.github.io/wasi-wheels/",
            "--extra-index-url",
            "https://pypi.org/simple",
            "--upgrade",
            ".",
        ])
        .current_dir(path)
        .assert()
        .success();

    Command::new("./.venv/bin/componentize-py")
        .args([
            "-w",
            "test",
            "componentize",
            "test_component.main",
            "-o",
            "main.wasm",
            "-p",
            "wasi_deps",
        ])
        .current_dir(path)
        .assert()
        .success();

    Command::new("wasmtime")
        .args(["run", "main.wasm"])
        .current_dir(path)
        .assert()
        .success();
}
