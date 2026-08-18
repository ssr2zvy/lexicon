# Test Container

Note: run all commands from the repository root, the folder that contains Cargo.toml and the containerization directory.

- `podman build -f containerization/test-container/Containerfile -t lexicon-local-test-image .` — build the image with the repo root as the build context. If you get a message like this, do not worry and wait for completion: level=warning msg="can't raise ambient capability CAP_SYS_CHROOT: operation not permitted"
- `podman run -d --name lexicon-local-test -v "$PWD":/lexicon --workdir /lexicon lexicon-local-test-image` — start the container and mount the repo at `/lexicon`.
- `podman start lexicon-local-test` — start a previously stopped container.
- `podman pause lexicon-local-test` — pause the container.
- `podman unpause lexicon-local-test` — resume the paused container.
- `podman stop lexicon-local-test` — stop the container.
- `podman rm -f lexicon-local-test` — remove the container.
- `podman rmi -f lexicon-local-test-image` — remove the image.

## Exec guide

Use `-it` only for an interactive shell. For a one-time command, use the normal exec form without `-it`.

- `podman exec -it lexicon-local-test bash` — open an interactive shell.
- `podman exec lexicon-local-test bash -lc '<command>'` — run a one-time command through a login shell.
- `podman exec lexicon-local-test bash -c '<command>'` — run a one-time command directly without a login shell.

Examples:

- `podman exec lexicon-local-test bash -lc 'cd /lexicon && cargo --version'`
- `podman exec lexicon-local-test bash -c 'cd /lexicon && cargo --version'`
- `podman exec lexicon-local-test bash -lc 'cd /lexicon && bash automation/build_bundle_install/build_bundle_install.sh'`
- `podman exec lexicon-local-test bash -c 'cd /lexicon && bash automation/build_bundle_install/build_bundle_install.sh'`

Both `bash -lc` and `bash -c` work in this image; the choice is just whether you want login-shell behavior or a direct command wrapper.

## Running `build_bundle_install.sh` / `install.sh` without `-it`

Both scripts drive the interactive `lexicon-bundle` menu, which reads from stdin. Plain `podman exec` (no `-i`/`-it`) gives the command a closed stdin, so any prompt gets an instant empty read and `install.sh` will spam "Invalid selection" forever instead of waiting.

The preferred, non-interactive-safe way to run these scripts is to pipe the menu answers directly into the command, inside the same `-lc`/`-c` string — this needs no `-i`/`-it` at all, since the pipe is built by the shell running inside the container, not by podman's own stdin attachment:

- Install path (one prompt, `1` to install):
  `podman exec lexicon-local-test bash -lc 'cd automation/build_bundle_install && echo 1 | bash install.sh'`
- Uninstall path (two prompts: `1` to choose Uninstall, then `y` to confirm):
  `podman exec lexicon-local-test bash -lc 'cd automation/build_bundle_install && printf "1\ny\n" | bash install.sh'`
- Full build/bundle/install pipeline, install path:
  `podman exec lexicon-local-test bash -lc 'cd /lexicon && echo 1 | bash automation/build_bundle_install/build_bundle_install.sh'`

This is the preferred way to drive these two scripts non-interactively; reserve `podman exec -it lexicon-local-test bash` for when you actually want to sit at the interactive menu yourself.

The key concept is the build context: the image must be built from the repo root, not from inside the containerization/test-container folder. This is why the build command includes the trailing `.` and the Containerfile copies `containerization/test-container/entrypoint.sh` from the repo root context.
