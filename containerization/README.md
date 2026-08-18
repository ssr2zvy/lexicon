# Containerization

This folder holds the container definitions used for local, containerized Lexicon workflows. There are two, with different purposes:

## `test-container/` — for developing, testing, and releasing Lexicon itself

Mounts the entire repository into the container at runtime and installs the full Rust/Zig toolchain (cargo, cargo-zigbuild, zig) in the image. Use this when you're working on the Lexicon source: building, running the build/bundle/install pipeline, or preparing a release artifact. See [test-container/README.md](test-container/README.md).

## `lexicon-container/` — for using Lexicon as a consumer

Takes a single, already-built bundle tar.xz (produced by the pipeline above) and bakes just that into the image, installing it non-interactively at build time. No Rust toolchain, no repo mount — just a running environment with the `lexicon` CLI installed and reachable on `PATH`, as an end user would have it. See [lexicon-container/README.md](lexicon-container/README.md).

## Shared concept: build context

Both Containerfiles must be built with the repository root as the build context (trailing `.`), not from inside their own folder, since they `COPY` files (an `entrypoint.sh`, and for `lexicon-container` also the bundle tar.xz) from paths relative to the repo root:

```bash
podman build -f containerization/test-container/Containerfile -t lexicon-local-test-image .
podman build -f containerization/lexicon-container/Containerfile --build-arg BUNDLE_TAR=<path-to-tar.xz> -t lexicon-local-image .
```
