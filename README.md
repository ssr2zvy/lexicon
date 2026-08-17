
# TOOLING
## bundling lexicon (using mza repo)
- git clone https://github.com/ssr2zvy/mza.git OR download the source zip into automation/build_and_bundle/; add mza/ to .gitignore
- copy `build_and_bundle/mza_artifact.toml`, created per mza documentation, into `build_and_bundle/mza/artifact.toml`; the current .toml will output created artifacts to `lexicon/artifacts/`
- Run the entrypoint script within `mza/`. As per mza documentation, the entrypoint is `make-artifact.sh` for Unix-based systems and `make-artifact.ps1` for Windows