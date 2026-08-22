# Continuous implementation workflow

## Purpose

Use a temporary working branch for rapid, frequent implementation commits and pushes. When the implementation is complete, squash the entire temporary branch into one commit on `main`, then delete the temporary branch.

This preserves detailed checkpoint history during development without adding every checkpoint commit to `main` or inflating the daily commit contribution count.

## Branches

- `main` is the permanent public history.
- `current_tracking` is the temporary working branch.
- All implementation work, tests, reports, checkpoint commits, and intermediate pushes occur on `current_tracking`.
- `current_tracking` must never become the repository’s default branch or the `gh-pages` branch.
- Do not normally merge `current_tracking` into `main`.
- Final integration must use a squash merge.

GitHub counts commits on the default branch or `gh-pages`. Intermediate commits confined to `current_tracking` do not count as daily commit contributions. The final squash commit on `main` counts as one commit.

## Starting or resuming work

Run from anywhere inside the repository:

```bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

remote="${CURRENT_REMOTE:-origin}"
working_branch="current_tracking"

git fetch --prune "$remote"
```

Determine the current branch:

```bash
current_branch="$(git branch --show-current)"
```

### If already on `current_tracking`

Confirm that the branch tracks the correct remote branch:

```bash
git branch --set-upstream-to="$remote/$working_branch" \
    "$working_branch" 2>/dev/null || true
```

If the working tree is clean, update it:

```bash
if [ -z "$(git status --porcelain)" ]; then
    git pull --ff-only "$remote" "$working_branch"
fi
```

If the working tree is not clean, do not pull, reset, stash, or discard anything. Inspect the existing changes and determine whether they belong to the active task before proceeding.

### If `current_tracking` exists remotely but is not checked out

The working tree must be clean before switching:

```bash
if [ -n "$(git status --porcelain)" ]; then
    echo "Working tree contains changes. Refusing to switch branches."
    git status --short
    exit 1
fi

git switch --track "$remote/$working_branch"
```

If the local branch already exists:

```bash
git switch "$working_branch"
git pull --ff-only "$remote" "$working_branch"
```

### If `current_tracking` does not exist

The working tree must be inspected first:

```bash
git status --short
```

If all existing changes belong to the implementation that should move onto the temporary branch, create the branch without discarding them:

```bash
git switch -c "$working_branch"
git push --set-upstream "$remote" "$working_branch"
```

If unrelated changes are present, stop and report them. Do not stash, reset, clean, or discard them automatically.

## Reading the task

Before implementation:

1. Read root `current.md` completely.
2. Treat `current.md` as the authoritative implementation task.
3. Inspect the actual source code before changing anything.
4. Preserve unrelated existing changes.
5. Prioritize concrete source-code implementation over prose.
6. Do not claim a flow works unless an executable test reaches it.
7. Do not modify `main` during active implementation.

## Implementing the task

Implement everything required by `current.md`.

After each coherent checkpoint:

1. Run the relevant tests.
2. Update `current.md` with the verified state.
3. Stage only the exact files belonging to the task.
4. Inspect the staged file list.
5. Commit and push the checkpoint to `current_tracking`.

Never use:

```bash
git add .
git add -A
```

Stage exact files instead:

```bash
git add -- current.md path/to/exact/source.rs path/to/exact/test.rs
```

Inspect the staged files:

```bash
git diff --cached --name-only
git diff --cached --check
```

Confirm that every staged file belongs to the current task. If an unrelated file is staged, unstage that exact file before committing:

```bash
git restore --staged -- path/to/unrelated-file
```

Commit and push:

```bash
git commit -m "current"
git push "$remote" "$working_branch"
```

Frequent checkpoint commits and pushes are allowed on `current_tracking`.

## Updating `current.md`

After each implementation checkpoint, replace `current.md` with a focused implementation report containing:

- The exact source files changed.
- The exact functions, types, or modules changed.
- The function flow from the public entrypoint to the implemented behavior.
- Corrections made to previous behavior.
- Tests and executable verification commands run.
- Exact test results.
- Remaining gaps or blockers.
- Whether the checkpoint is complete or requires another implementation step.

The report must describe the implemented source code. Do not replace it with a generic Git-status report, task ledger, or workflow explanation.

Commit and push the updated `current.md` together with the exact implementation files for that checkpoint.

## Handling concurrent remote updates

Before pushing:

```bash
git fetch --prune "$remote"

local_head="$(git rev-parse HEAD)"
remote_head="$(git rev-parse "$remote/$working_branch")"
```

If `remote_head` is not an ancestor of `local_head`, the remote branch contains work that is not in the local branch:

```bash
if ! git merge-base --is-ancestor "$remote_head" "$local_head"; then
    echo "Remote current_tracking changed independently."
    echo "Local:  $local_head"
    echo "Remote: $remote_head"
    exit 1
fi
```

Stop and report the divergence. Do not force-push, merge, reset, or rebase automatically.

## Finishing the implementation

Do not integrate into `main` until explicitly instructed that the implementation is complete.

Before final integration:

1. Confirm all task changes are committed on `current_tracking`.
2. Confirm the working tree is clean.
3. Fetch the latest remote state.
4. Confirm tests pass from the final `current_tracking` commit.
5. Confirm `current.md` contains the final verified implementation report.

Run:

```bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

remote="${CURRENT_REMOTE:-origin}"
working_branch="current_tracking"

if [ "$(git branch --show-current)" != "$working_branch" ]; then
    echo "Expected to be on $working_branch."
    exit 1
fi

if [ -n "$(git status --porcelain)" ]; then
    echo "Working tree is not clean."
    git status --short
    exit 1
fi

git fetch --prune "$remote"
git pull --ff-only "$remote" "$working_branch"
```

Run the complete relevant test suite before integration.

## Squashing into `main`

After explicit approval to finalize:

```bash
git switch main
git pull --ff-only "$remote" main
git merge --squash "$working_branch"
```

Inspect the complete squashed change:

```bash
git status --short
git diff --cached --stat
git diff --cached --check
```

Then create exactly one permanent commit:

```bash
git commit -m "Implement completed Lexicon change"
git push "$remote" main
```

This creates one commit on `main`, regardless of how many checkpoint commits existed on `current_tracking`.

Do not use any of these integration methods:

```bash
git merge current_tracking
git merge --no-ff current_tracking
git rebase current_tracking
```

Those methods can preserve or introduce the individual checkpoint commits into `main`. Final integration must use:

```bash
git merge --squash current_tracking
```

## Developer attribution

The final squash commit’s author receives commit credit.

If several developers contributed and should all receive attribution, add one trailer per additional developer to the final commit message:

```text
Co-authored-by: Developer Name <email-associated-with-their-GitHub-account>
```

The result remains one commit on `main`, even when multiple developers are credited.

## Deleting the temporary branch

Only after the squash commit has been successfully pushed to `main`:

```bash
git push "$remote" --delete "$working_branch"
git branch -D "$working_branch"
```

Deleting the temporary branch removes its normal public branch reference. It is cleanup; the squash merge is what prevents the checkpoint commits from entering `main`.

## Contribution-count result

If 30 checkpoint commits are pushed to `current_tracking` and then squash-merged:

- The 30 temporary commits do not count as default-branch commit contributions.
- One new squash commit is added to `main`.
- The final result counts as one commit contribution.
- A normal merge would instead bring the original commits into `main` and could count all of them.
- Do not create a pull request if avoiding additional pull-request activity on the contribution graph is also desired; perform the approved squash integration directly with Git.

## Final response after each checkpoint

Report:

- That `current.md` was updated.
- The new `current_tracking` commit hash.
- The exact files committed.
- The tests run and their results.
- Any remaining implementation work.

## Final response after squash integration

Report:

- The final `main` squash commit hash.
- The number of temporary commits consolidated.
- The tests run and their results.
- Confirmation that `current_tracking` was deleted locally and remotely.
- Confirmation that only one implementation commit was added to `main`.