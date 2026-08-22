CURRENT.MD CONTINUOUS IMPLEMENTATION RULE

The user commits implementation instructions only to root current.md.
Your job is to retrieve that file, implement its instructions in the existing
dirty working directory, replace current.md with a verified implementation
report, and commit/push only current.md.

The source-code changes must remain uncommitted in the main working directory.

PHASE 1 — RETRIEVE ONLY CURRENT.MD

Run from anywhere inside the repository:

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

local_branch="$(git branch --show-current)"
if [ -z "$local_branch" ]; then
    echo "Cannot proceed from detached HEAD." >&2
    exit 1
fi

upstream="$(git rev-parse --abbrev-ref --symbolic-full-name '@{upstream}')"
remote="$(git config --get "branch.${local_branch}.remote")"
merge_ref="$(git config --get "branch.${local_branch}.merge")"
remote_branch="${merge_ref#refs/heads/}"

git fetch "$remote" "$remote_branch"

instruction_commit="$(git rev-parse "$upstream")"
state_file="$(git rev-parse --git-path lexicon-current-instruction)"
printf '%s\n' "$instruction_commit" > "$state_file"

unexpected_remote_changes="$(
    git diff --name-only HEAD "$instruction_commit" -- |
    grep -v '^current\.md$' || true
)"

if [ -n "$unexpected_remote_changes" ]; then
    echo "Remote changes exist outside current.md:" >&2
    printf '%s\n' "$unexpected_remote_changes" >&2
    echo "Stop without changing the working directory." >&2
    exit 1
fi

git restore \
    --source="$instruction_commit" \
    --worktree \
    -- current.md

Do not use --staged.

Do not run git pull, reset, checkout, switch, stash, clean, rebase, or merge.

The restore intentionally replaces only the working-tree copy of current.md.
Every other staged, unstaged, and untracked change must remain untouched.

PHASE 2 — IMPLEMENT

1. Read root current.md completely.
2. Treat it as the authoritative implementation task.
3. Inspect the real source code before making changes.
4. Prioritize concrete source-code implementation over prose.
5. Preserve all unrelated working-directory changes.
6. Implement and test everything required by current.md.
7. Do not commit or push any source-code changes.
8. Do not claim behavior works unless an executable test reaches it.
9. Inspect git status before and after implementation to confirm unrelated
   changes remain present.

PHASE 3 — WRITE THE REPORT

Replace current.md with a fresh report containing:

1. Exact source files changed.
2. Exact functions/types changed.
3. The implemented function-level call flow.
4. Concrete source-code excerpts for important behavior.
5. Tests added or changed.
6. Exact verification commands.
7. Exact pass/fail results.
8. Remaining gaps or unimplemented behavior.
9. Confirmation that source-code changes remain uncommitted.

Do not place unverified claims in the report.

PHASE 4 — ENSURE THE INSTRUCTIONS WERE NOT REPLACED CONCURRENTLY

Run:

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

local_branch="$(git branch --show-current)"
upstream="$(git rev-parse --abbrev-ref --symbolic-full-name '@{upstream}')"
remote="$(git config --get "branch.${local_branch}.remote")"
merge_ref="$(git config --get "branch.${local_branch}.merge")"
remote_branch="${merge_ref#refs/heads/}"

state_file="$(git rev-parse --git-path lexicon-current-instruction)"
instruction_commit="$(cat "$state_file")"

git fetch "$remote" "$remote_branch"

latest_upstream_commit="$(git rev-parse "$upstream")"

if [ "$latest_upstream_commit" != "$instruction_commit" ]; then
    echo "current.md changed remotely while implementation was running." >&2
    echo "Do not overwrite or push the report." >&2
    exit 1
fi

PHASE 5 — COMMIT ONLY CURRENT.MD FROM AN ISOLATED WORKTREE

The main working directory must not be used for this commit.

Run:

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

local_branch="$(git branch --show-current)"
upstream="$(git rev-parse --abbrev-ref --symbolic-full-name '@{upstream}')"
remote="$(git config --get "branch.${local_branch}.remote")"
merge_ref="$(git config --get "branch.${local_branch}.merge")"
remote_branch="${merge_ref#refs/heads/}"

state_file="$(git rev-parse --git-path lexicon-current-instruction)"
instruction_commit="$(cat "$state_file")"

report_copy="$(mktemp)"
cp -- current.md "$report_copy"

report_worktree="$(mktemp -d)"
rmdir "$report_worktree"

cleanup_report_worktree() {
    git -C "$repo_root" worktree remove --force "$report_worktree" \
        >/dev/null 2>&1 || true
    rm -f "$report_copy"
}
trap cleanup_report_worktree EXIT

git worktree add --detach "$report_worktree" "$instruction_commit"

cp -- "$report_copy" "$report_worktree/current.md"

git -C "$report_worktree" add -- current.md

staged_files="$(
    git -C "$report_worktree" diff --cached --name-only
)"

if [ "$staged_files" != "current.md" ]; then
    echo "Refusing to commit because the isolated commit contains:" >&2
    printf '%s\n' "$staged_files" >&2
    exit 1
fi

git -C "$report_worktree" commit -m "current"

report_commit="$(
    git -C "$report_worktree" rev-parse HEAD
)"

git -C "$report_worktree" push \
    "$remote" \
    "HEAD:refs/heads/$remote_branch"

rm -f "$state_file"

echo "current.md updated and pushed."
echo "Report commit: $report_commit"

PHASE 6 — RETURN TO THE USER

Return:

- `current.md updated and pushed`
- The report commit hash.
- The exact source files changed but left uncommitted.
- The verification commands and results.
- Any remaining implementation gaps.

Do not say the task is complete unless the push succeeded.