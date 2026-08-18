
# Automation
## bundling lexicon (using mza repo)
- git clone https://github.com/ssr2zvy/mza.git OR download the source zip into automation/build_and_bundle/; add mza/ to .gitignore
- copy `build_and_bundle/mza_artifact.toml`, created per mza documentation, into `build_and_bundle/mza/artifact.toml`; the current .toml will output created artifacts to `lexicon/artifacts/`
- Run the entrypoint script within `mza/`. As per mza documentation, the entrypoint is `make-artifact.sh` for Unix-based systems and `make-artifact.ps1` for Windows

# Test Container

The local container workflow is documented in [test-container/README.md](test-container/README.md).

Use the repository root as the Podman build context, not the `test-container` folder itself. This is the important concept behind the working build command:

- `podman build -f test-container/Containerfile -t lexicon-local-image .`

This keeps the repo available to the builder and allows the Containerfile to copy `test-container/entrypoint.sh` from the root context correctly while the container itself runs with the repo mounted at `/lexicon`.