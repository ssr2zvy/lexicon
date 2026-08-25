# Continuous implementation workflow

1. Fetch the latest remote state.
   ```bash
   git fetch
   ```

2. Ensure the current branch is `current_tracking`.
   - If the current branch is not `current_tracking`, switch to it.
   ```bash
   git switch current_tracking
   ```

3. Pull the latest changes for `current_tracking`.
   ```bash
   git pull --ff-only origin current_tracking
   ```

4. Ensure the working directory is clean for fresh project validation.
   - If a stale `telugu-lexicon/` directory exists in the working directory, delete it before continuing.
   - Initialize a fresh Lexicon project in the working directory with the project name `telugu-lexicon` and the sources directory named `sources/`.
   - Example:
   ```bash
   rm -rf telugu-lexicon
   lexicon init . telugu-lexicon
   ```
   - The resulting project root must be `./telugu-lexicon` and the configured sources directory must be `./telugu-lexicon/sources`.

5. Read `current.md` and plan the implementation.
   - Determine how you will complete the implementation request described in `current.md`.

6. Implement the required source changes.
   - Keep the implementation aligned with the plan in Step 5.
   - Do not pivot to a new strategy without clear confirmation from the user if the plan is not producing the intended result.

7. Validate using the standard Cargo workflow.
   ```bash
   cargo check --workspace
   cargo test --workspace --quiet
   ```
   - Prefer the normal Cargo validation commands over the bundle/install script when the script is unavailable or fails in this environment.
   - If the standard Cargo validation succeeds, record the exact command output and any relevant warnings.
   - If the validation fails, report the exact failing build or test step.
   - If this flow does not lead to the intended result, stop and tell the user exactly what failed; do not pivot to a different strategy without explicit confirmation.

8. Replace `current.md` with your implementation report.
   - Write the implementation report based on the work completed in Steps 4 through 6.

9. Commit and push the updated report.
   ```bash
   git add -A
   git commit -m "update current report"
   git push
   ```

10. Report to the user.
   - Share the outcome, the validation result, and any remaining blockers.
