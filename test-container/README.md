# Test Container

Note: run all commands from the repository root, the folder that contains Cargo.toml and the test-container directory.

- `podman build -f test-container/Containerfile -t lexicon-local-image .` — build the image with the repo root as the build context.
- `podman run -d --name lexicon-local -v "$PWD":/lexicon --workdir /lexicon lexicon-local-image` — start the container and mount the repo at `/lexicon`.
- `podman start lexicon-local` — start a previously stopped container.
- `podman exec -it lexicon-local bash` — open a shell inside the running container.
- `podman pause lexicon-local` — pause the container.
- `podman unpause lexicon-local` — resume the paused container.
- `podman stop lexicon-local` — stop the container.
- `podman rm -f lexicon-local` — remove the container.
- `podman rmi -f lexicon-local-image` — remove the image.

The key concept is the build context: the image must be built from the repo root, not from inside the test-container folder. This is why the build command includes the trailing `.` and the Containerfile copies `test-container/entrypoint.sh` from the repo root context.
