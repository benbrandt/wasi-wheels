//! Prep cpython wasi build

use wasi_wheels::download_and_compile_cpython;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    download_and_compile_cpython().await
}
