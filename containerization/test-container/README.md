# Test Container

Note: run all commands from the repository root, the folder that contains
Cargo.toml and the containerization directory.

- `podman build -f containerization/test-container/Containerfile -t lexicon-local-test-image .`
  build the image with the repo root as the build context. If you get
  `level=warning msg="can't raise ambient capability CAP_SYS_CHROOT: operation not permitted"`,
  that's normal; wait for completion.
- `podman run -d --name lexicon-local-test -v "$PWD":/lexicon --workdir /lexicon lexicon-local-test-image`
  start the container and mount the repo at `/lexicon`.
- `podman start / pause / unpause / stop / rm / rmi` as usual.

## Exec guide

Use `-it` only for an interactive shell. For a one-time command, use the
standard exec form without `-it`.

- `podman exec -it lexicon-local-test bash` open an interactive shell.
- `podman exec lexicon-local-test bash -lc '<command>'` run a one-time
  command through a login shell.
- `podman exec lexicon-local-test bash -c '<command>'` run a one-time
  command directly without a login shell.

Examples:

```
podman exec lexicon-local-test bash -lc 'cd /lexicon && cargo --version'
podman exec lexicon-local-test bash -lc 'cd /lexicon && cargo test --workspace --locked'
podman exec lexicon-local-test bash -lc 'cd /lexicon && bash automation/build_bundle_mza/build_release.sh'
```

Both `bash -lc` and `bash -c` work in this image; the choice is just
whether you want login-shell behavior or a direct command wrapper.

## Running `build_release.sh` (the locked non-interactive pipeline)

`automation/build_bundle_mza/build_release.sh` is the only authoritative
release entrypoint after RELEASE-02 removed `automation/build_bundle_install/`.
The script refuses to run while the `<accepted-mza-sha>` placeholder
disagrees with the MZA submodule's gitlink and never prompts the operator.
Run it inside this container via:

```
podman exec lexicon-local-test bash -lc 'cd /lexicon && bash automation/build_bundle_mza/build_release.sh'
```

Reserve `podman exec -it lexicon-local-test bash` for when you actually
want to sit at a shell yourself.

The image must be built from the repo root, not from inside
`containerization/test-container/`. This is why the build command
includes the trailing `.` and the Containerfile copies
`containerization/test-container/entrypoint.sh` from the repo root context.
