use std::path::{Path, PathBuf};

use heck::ToSnakeCase;
use tokio::process::Command;

use crate::run;

use super::{PythonVersion, SupportedProjects};

/// Wheel tag at the moment
const PLATFORM_TAG: &str = "wasi_0_0_0_wasm32";

/// Add environment variables for cross-compilation
pub fn default_wheel_flags<'a>(
    command: &'a mut Command,
    python_version: PythonVersion,
    package_dir: impl AsRef<Path>,
    path_variable: &str,
) -> &'a mut Command {
    let cross_prefix = python_version.cross_prefix();
    let wasi_sdk_path = python_version.wasi_sdk_path();
    let cc = wasi_sdk_path.join("bin/clang");

    command
        .current_dir(&package_dir)
        // Make it possible to not have to activate the venv
        .env("PATH", path_variable)
        .env("CROSS_PREFIX", &cross_prefix)
        .env("WASI_SDK_PATH", &wasi_sdk_path)
        .env("SYSCONFIG", python_version.cross_lib_dir())
        .env("CC", &cc)
        .env("CXX", wasi_sdk_path.join("bin/clang++"))
        .env(
            "PYTHONPATH",
            cross_prefix.join(format!("lib/python{python_version}")),
        )
        .env(
            "CFLAGS",
            format!(
                "-I{}/include/python{python_version} -D__EMSCRIPTEN__=1",
                cross_prefix.to_str().unwrap()
            ),
        )
        .env(
            "CXXFLAGS",
            format!(
                "-I{}/include/python{python_version}",
                cross_prefix.to_str().unwrap()
            ),
        )
        .env("LDSHARED", cc)
        .env("AR", wasi_sdk_path.join("bin/ar"))
        .env("RANLIB", "true")
        .env("LDFLAGS", "-shared")
        .env(
            "_PYTHON_SYSCONFIGDATA_NAME",
            "_sysconfigdata__wasi_wasm32-wasi",
        )
}

pub fn wheel_path(
    project: SupportedProjects,
    python_version: PythonVersion,
    package_dir: impl AsRef<Path>,
    version: &str,
) -> PathBuf {
    package_dir.as_ref().join(format!(
        "dist/{project}-{version}-cp{py_version}-cp{py_version}-{PLATFORM_TAG}.whl",
        project = project.to_string().to_snake_case(),
        py_version = python_version.to_string().replace('.', "")
    ))
}

pub async fn retag_any_wheel(
    project: SupportedProjects,
    python_version: PythonVersion,
    package_dir: impl AsRef<Path>,
    version: &str,
    path_variable: &str,
) -> anyhow::Result<()> {
    let wheel = wheel_path(project, python_version, package_dir, version);
    // Rewrite the wheel to the correct target
    run(Command::new("pip")
        .args(["install", "wheel", "--upgrade"])
        // Make it possible to not have to activate the venv
        .env("PATH", path_variable))
    .await?;
    run(Command::new("wheel")
        .args([
            "tags",
            "--platform-tag",
            PLATFORM_TAG,
            "--remove",
            // Maturin outputs a wheel with `any` platform tag
            &wheel.to_string_lossy().replace(PLATFORM_TAG, "any"),
        ])
        // Make it possible to not have to activate the venv
        .env("PATH", path_variable))
    .await?;
    Ok(())
}
