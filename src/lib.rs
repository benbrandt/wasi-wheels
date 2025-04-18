//! Tooling to generate Python wheels usable in WASI contexts and consumable as a Python registry.

use std::iter;

use anyhow::{Context, bail};
use tokio::process::Command;

mod build;
mod index;

pub use build::{PythonVersion, SupportedProjects, build_and_publish, install_build_tools};
pub use index::{download_package, generate_index};

/// Run a given command with common error handling behavior
///
/// # Errors
///
/// Returns error if the command fails for any reason.
pub async fn run(command: &mut Command) -> anyhow::Result<()> {
    let command_string = iter::once(command.as_std().get_program())
        .chain(command.as_std().get_args())
        .map(|arg| arg.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");

    let status = command.status().await.with_context({
        let command_string = command_string.clone();
        move || command_string
    })?;

    if status.success() {
        Ok(())
    } else {
        bail!("command `{command_string}` failed",);
    }
}
