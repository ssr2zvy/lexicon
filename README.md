
# Automation
## bundling lexicon (using mza repo)
- git clone https://github.com/ssr2zvy/mza.git OR download the source zip into automation/build_bundle_mza/; add mza/ to .gitignore
- copy `build_bundle_mza/mza_artifacts.toml`, created per mza documentation, into `build_bundle_mza/mza/artifact.toml`; the current .toml will output created artifacts to `lexicon/artifacts/`
- Run the entrypoint script within `mza/`. As per mza documentation, the entrypoint is `make-artifact.sh` for Unix-based systems and `make-artifact.ps1` for Windows

# Containerization

Container definitions live under [containerization/](containerization/):

- [containerization/test-container/README.md](containerization/test-container/README.md) — for developing, testing, and releasing the Lexicon project itself. Mounts the whole repo and builds Lexicon from source, giving you a persistent, working Rust/Zig toolchain.
- [containerization/lexicon-container/README.md](containerization/lexicon-container/README.md) — for using the Lexicon project. Bakes a single, already-built bundle tar.xz into the image and installs it at build time; never needs the Rust toolchain at runtime.

Use the repository root as the Podman build context, not the container's own folder. This is the important concept behind the working build commands:

- `podman build -f containerization/test-container/Containerfile -t lexicon-local-test-image .`
- `podman build -f containerization/lexicon-container/Containerfile --build-arg BUNDLE_TAR=<path-to-tar.xz> -t lexicon-local-image .`

This keeps the repo available to the builder and allows each Containerfile to copy its own `entrypoint.sh` from the root context correctly.