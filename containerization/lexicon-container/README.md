# Lexicon Container

Note: run all commands from the repository root, the folder that contains Cargo.toml and the containerization directory.

Unlike `test-container` (which mounts the whole repo and builds Lexicon from source), `lexicon-container` bakes a single, already-built bundle tar.xz into the image and installs it at build time. It never needs the Rust toolchain at runtime.

## The `BUNDLE_TAR` build arg

`BUNDLE_TAR` is a path *relative to the build context* pointing at a `lexicon-bundle-*.tar.xz` produced by `automation/build_and_bundle/mza` (via `automation/build_bundle_install/build_bundle_install.sh`). It gets substituted directly into the Containerfile's `COPY ${BUNDLE_TAR} ...` instruction, so it must exist and be reachable from the build context root. The Containerfile fails the build immediately with a clear error if `BUNDLE_TAR` isn't supplied.

Because the build context is the repo root (`lexicon/` itself, hence the trailing `.` in the build command), the value is the path from the repo root down to the tar.xz, for example:

```
artifacts/lexicon_bundle/custom/cargo-bundler-v0.1.0/0.1.0-LOCALSNAPSHOT/x86_64-unknown-linux-musl/lexicon-bundle-0.1.0-LOCALSNAPSHOT-x86_64-unknown-linux-musl.tar.xz
```

That pattern is `artifacts/lexicon_bundle/<type>/<protocol>/<version>/<target>/lexicon-bundle-<version>-<target>.tar.xz`. Run `automation/build_bundle_install/build_bundle_install.sh` first if this file doesn't exist yet.

## Commands

- `podman build -f containerization/lexicon-container/Containerfile --build-arg BUNDLE_TAR=<path-to-tar.xz> -t lexicon-local-image .` — build the image with the repo root as the build context, installing the given bundle at build time. Omitting `--build-arg BUNDLE_TAR=...` fails the build immediately with an error instead of copying the wrong (or no) file. If you get a message like this, do not worry and wait for completion: level=warning msg="can't raise ambient capability CAP_SYS_CHROOT: operation not permitted"
- `podman run -d --name lexicon-local lexicon-local-image` — start the container; it installs once at build time and then stays idle.
- `podman start lexicon-local` — start a previously stopped container.
- `podman pause lexicon-local` — pause the container.
- `podman unpause lexicon-local` — resume the paused container.
- `podman stop lexicon-local` — stop the container.
- `podman rm -f lexicon-local` — remove the container.
- `podman rmi -f lexicon-local-image` — remove the image.

## Exec guide

Use `-it` only for an interactive shell. For a one-time command, use the normal exec form without `-it`.

- `podman exec -it lexicon-local bash` — open an interactive shell.
- `podman exec lexicon-local bash -lc '<command>'` — run a one-time command through a login shell.
- `podman exec lexicon-local bash -c '<command>'` — run a one-time command directly without a login shell.

Examples:

- `podman exec lexicon-local bash -lc 'lexicon -V'`
- `podman exec lexicon-local bash -c 'lexicon -V'`

The key concept is the build context: the image must be built from the repo root, not from inside the lexicon-container folder. This is why the build command includes the trailing `.` and the Containerfile copies `containerization/lexicon-container/entrypoint.sh` and the `BUNDLE_TAR` path from the repo root context.
