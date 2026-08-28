# Continuous implementation workflow

This workflow runs in a loop. Each iteration executes one `current.md`, merges the result into `main`, then derives the next `current.md` from `workspace/specs/contract.md` and `workspace/specs/specs.md` (and `workspace/specs/issues.md` when relevant). The loop repeats until the project fully satisfies the contract and specs, with no remaining gap left to correct.

Authority order:

1. This file defines the workflow steps and the loop mechanics.
2. `workspace/specs/contract.md` and `workspace/specs/specs.md` are the ultimate authority on what "complete" means and what the next `current.md` must contain.
3. `current.md` defines the actual implementation request, constraints, exclusions, and completion criteria for the current iteration only.
4. Always route planning, implementation scope, validation policy, and the final report for the current iteration through `current.md`.
5. If any general step below conflicts with `current.md`, follow `current.md` for the current iteration. If `current.md` conflicts with the contract or specs, flag this explicitly in the report instead of silently resolving it.

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

- On a Windows host, `podman` runs client-side against a podman machine (a WSL-backed VM). If a bare `podman exec ...` on the host does not reach the running container, route the same command through `podman machine ssh`:

```bash
podman machine ssh <machine-name> "podman exec lexicon-local-test bash -lc 'cd /lexicon && cargo test --workspace --quiet'"
```

  - Discover the machine name with `podman machine list`.
  - The repository is reachable inside the machine's WSL filesystem (e.g. `/mnt/c/Users/<user>/...`), but prefer driving Cargo through the already-mounted `lexicon-local-test` container path (`/lexicon`) shown above rather than operating on the WSL-mounted path directly.
  - When output is large enough that a terminal-attached command risks truncation, redirect stdout/stderr to a log file and inspect the file afterward instead of relying on terminal output. On Windows/PowerShell, explicitly force UTF-8 output (e.g. `Out-File -Encoding utf8`) — PowerShell's default `*>`/`>` redirection writes UTF-16LE, which common text-processing tools cannot parse.
  - Unless `current.md` or the user says otherwise, the agent may run this containerized validation directly instead of asking the user to run it.

- If the workspace test run passes, validation is complete unless `current.md` defines a narrower validation policy.
- If the command fails, report the exact failing crate or step.
- If the bundle/install helper script is unavailable or cannot complete in this environment, fall back to the containerized Cargo validation flow with build and test, instead of using the custom install path.
- If `current.md` forbids Cargo, checks, builds, tests, CLI execution, runtime execution, or related validation for this milestone, do not run them. Record that validation was deferred exactly as `current.md` requires.
- Do not pivot to a different strategy without explicit confirmation if containerized Cargo validation itself is failing.

## 8. Replace `current.md` with your implementation report

- Write the implementation report based on the work completed against the previous `current.md` contract.
- Include every completion-report field required by that `current.md`.
- Confirm constraints and exclusions from that `current.md`, including any ban on host-local toolchain use and any deferred validation.

## 9. Commit and push the feature branch

```bash
git add -A
git commit -m "update current report"
git push -u origin <feature-branch-name>
```

- If a merge request/pull request workflow is available, open it from the feature branch targeting `main`.
- Do not merge into `main` if the `current.md` completion criteria for this iteration were not met, or if validation failed and was not explicitly deferred by `current.md`. Stop and report instead.

## 10. Merge the feature branch into `main`

- Once the iteration's `current.md` completion criteria are satisfied (validation passed, or was explicitly deferred per `current.md`), merge the feature branch into `main`.
- Prefer merging through the pull/merge request created in Step 9 when that workflow is available.
- Otherwise, merge locally and push:

```bash
git switch main
git pull --ff-only origin main
git merge --no-ff <feature-branch-name> -m "merge: <feature-branch-name>"
git push origin main
```

- This merge carries the just-written implementation report (the replaced `current.md`) onto `main`, alongside the source changes.

## 11. Derive the next `current.md` from the contract and specs

- Read `workspace/specs/contract.md` and `workspace/specs/specs.md` in full (and `workspace/specs/issues.md` when it tracks open work).
- Compare them against the current state of the source tree on `main`, including the implementation report that Step 10 just merged.
- Identify the next concrete, well-scoped gap between the implementation and the contract/specs: a missing guarantee, an unmet requirement, an unresolved defect, or the next milestone the contract implies.
- Write a new `current.md` on `main` describing that gap as a corrective or additive milestone, using the same structure as prior `current.md` iterations (objective, contract authority, repository-grounded defects or requirements, required corrections, preserved behavior, explicit exclusions, command-execution constraints, and completion-report requirements).
- Scope each new `current.md` to one coherent milestone. Do not try to close every remaining gap in a single iteration.
- If, after this comparison, the implementation already fully satisfies `workspace/specs/contract.md` and `workspace/specs/specs.md` with no remaining gap, do not write a new corrective `current.md`. Instead:
  - write a final `current.md` (or a completion note in its place) stating that the project is complete against the contract and specs, with a rationale;
  - commit and push this final state directly to `main`;
  - stop the loop and report completion to the user.

## 12. Commit and push the new `current.md` on `main`

```bash
git add current.md
git commit -m "plan: next current.md milestone"
git push origin main
```

## 13. Loop

- Unless Step 11 concluded the project is complete, return to Step 1 and begin a new iteration against the `current.md` just pushed in Step 12.
- Each iteration uses a freshly created feature branch from the up-to-date `main` (Step 2), so branches are never reused across iterations.

## 14. Report to the user

- After each iteration (Steps 1 through 12 or 13), share the outcome, the validation result or explicitly deferred validation per that iteration's `current.md`, whether the feature branch was merged into `main`, and any remaining blockers.
- When Step 11 concludes the project is complete, report that explicitly and stop the loop instead of proceeding to Step 13.
- If a blocker prevents completing Steps 9 through 12 (for example, missing merge permissions), stop the loop, report the blocker, and wait for the user before resuming.
