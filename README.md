# wasi-wheels

Compile Python wheels for use with WASI targets. Specifically, generate wheels that can be consumed by [componentize-py](https://github.com/bytecodealliance/componentize-py).

**This is very much a work in progress**. The goal is to get to a point where wheels are built and stored as GitHub releases, and provided through an alternate Python package registry for WASI builds until PyPi supports them natively.

Right now this tooling can:

- Setup the necessary build tooling
- Download an sdist build for a given project and version
- Build wheels for specific packages
- Upload wheels to GitHub releases
- Provide a registry for installation

| **Supported Wheel**                                      | **Versions**  |
| -------------------------------------------------------- | ------------- |
| [pydantic-core](https://pypi.org/project/pydantic-core/) | >= 2.18.3     |
| [regex](https://pypi.org/project/regex/)                 | >= 2021.11.10 |

## Use a wheel

If you want to use a wheel for use with componentize-py, you can run the following:

```sh
python3 -m pip install --target wasi_deps --platform any --platform wasi_0_0_0_wasm32 --python-version "3.12" --only-binary :all: --index-url https://benbrandt.github.io/wasi-wheels/ --extra-index-url https://pypi.org/simple --upgrade .
```

Then you can run your componentize-py build like so:

```sh
componentize-py -w world componentize skill_module -o output_file -p . -p wasi_deps
```

## Setup

Make sure you have `python3.12` or `python3.13` and [`rustup`](https://www.rust-lang.org/learn/get-started) installed.

After cloning the repo, you can run:

```sh
cargo run -- install-build-tools
```

This will setup [WASI SDK v24](https://github.com/WebAssembly/wasi-sdk) with some minor patches for compiling CPython (mainly making wasip2 look like wasi for now, until better support for the correct target is available).

It also pulls down Cpython for 3.12 and 3.13 and compiles it for wasi.

## Building a wheel locally

If you need to debug a build locally, you can run:

```sh
cargo run -- build <project> <version>
```

## Building the Index locally

```sh
cargo run -- generate-index benbrandt/wasi-wheels
```
