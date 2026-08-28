# Continuous implementation workflow

Authority order:

1. This file defines the workflow steps.
2. `current.md` defines the actual implementation request, constraints, exclusions, and completion criteria for this run.
3. Always route planning, implementation scope, validation policy, and the final report through `current.md`.
4. If any general step below conflicts with `current.md`, follow `current.md`.

## 1. Fetch the latest remote state

```bash
git fetch
```

## 2. Ensure you make your feature branch from `main`

- If the current branch is not `main`, switch to it.

```bash
git switch main
```

## 3. Pull the latest changes for `main` before making the feature branch

```bash
git pull --ff-only origin main
```

## 4. Ensure the working directory is clean for fresh project validation

- If a stale `telugu-lexicon/` directory exists in the working directory, delete it before continuing.
- Initialize a fresh Lexicon project in the working directory with the project name `telugu-lexicon` and the sources directory named `sources/`.
- Example:

```bash
rm -rf telugu-lexicon
lexicon init . telugu-lexicon
```

- The resulting project root must be `./telugu-lexicon` and the configured sources directory must be `./telugu-lexicon/sources`.
- Skip or adapt this step when `current.md` says the milestone is source-only, forbids CLI execution, or otherwise changes project-setup requirements.

## 5. Route through `current.md`

- Read `current.md` first and treat it as the task contract for this run.
- Plan only the work described in `current.md`.
- Honor its required corrections, preserved behavior, explicit exclusions, public/internal API boundary, and completion-report requirements.
- Do not expand scope beyond `current.md`.
- Do not begin excluded follow-on work such as background supervision, operator host, lexicon build, or automatic build-before-run unless `current.md` explicitly asks for it.

## 6. Implement the required source changes

- Keep the implementation aligned with the plan derived from `current.md`.
- Do not pivot to a new strategy without clear confirmation from the user if the plan is not producing the intended result.

## 7. Validate with Cargo only through the test container

- Never run host-local `cargo`, `rustc`, or equivalent toolchain commands directly on the host.
- Run every Cargo command through the repository test container.
- Work from the repository root (the folder that contains `Cargo.toml` and `containerization/`).
- Preferred container name/image from `containerization/test-container/README.md`:
  - image: `lexicon-local-test-image`
  - container: `lexicon-local-test`
- Ensure the test container is available before validation:
  - build if needed:

```bash
podman build -f containerization/test-container/Containerfile -t lexicon-local-test-image .
```

  - start if needed:

```bash
podman run -d --name lexicon-local-test -v "$PWD":/lexicon --workdir /lexicon lexicon-local-test-image
```

  - or start an existing container:

```bash
podman start lexicon-local-test
```

- Run Cargo only via `podman exec`, for example:

```bash
podman exec lexicon-local-test bash -lc 'cd /lexicon && cargo test --workspace --quiet'
```

- Other Cargo invocations must use the same pattern:

```bash
podman exec lexicon-local-test bash -lc 'cd /lexicon && cargo check'
podman exec lexicon-local-test bash -lc 'cd /lexicon && cargo build'
podman exec lexicon-local-test bash -lc 'cd /lexicon && cargo test -p <crate>'
```

- If the workspace test run passes, validation is complete unless `current.md` defines a narrower validation policy.
- If the command fails, report the exact failing crate or step.
- If the bundle/install helper script is unavailable or cannot complete in this environment, fall back to the containerized Cargo validation flow with build and test, instead of using the custom install path.
- If `current.md` forbids Cargo, checks, builds, tests, CLI execution, runtime execution, or related validation for this milestone, do not run them. Record that validation was deferred exactly as `current.md` requires.
- Do not pivot to a different strategy without explicit confirmation if containerized Cargo validation itself is failing.

## 8. Replace `current.md` with your implementation report

- Write the implementation report based on the work completed against the previous `current.md` contract.
- Include every completion-report field required by that `current.md`.
- Confirm constraints and exclusions from that `current.md`, including any ban on host-local toolchain use and any deferred validation.

## 9. Commit and push the updated report and/or make a merge request from the feature branch

```bash
git add -A
git commit -m "update current report"
git push
```

## 10. Report to the user

- Share the outcome, the validation result or explicitly deferred validation per `current.md`, and any remaining blockers.
- Stop when the `current.md` completion criteria are satisfied.
