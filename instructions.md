# Continuous implementation workflow

1. Fetch the latest remote state.
   ```bash
   git fetch
   ```

2. Ensure you make your copilot feature branch from `main`.
   - If the current branch is not `current_tracking`, switch to it.
   ```bash
   git switch main
   ```

3. Pull the latest changes for `main before making the feature branch.
   ```bash
   git pull --ff-only origin main
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

7. Validate using normal Cargo commands.
   ```bash
   cargo test --workspace --quiet
   ```
   - If the workspace test run passes, validation is complete.
   - If the command fails, report the exact failing crate or step.
   - If the bundle/install helper script is unavailable or cannot complete in this environment, fall back to the standard Cargo validation flow with build and test, instead of using the custom install path.
   - Do not pivot to a different strategy without explicit confirmation if the Cargo validation itself is failing.

8. Replace `current.md` with your implementation report.
   - Write the implementation report based on the work completed in Steps 4 through 6.

9. Commit and push the updated report and/or make a merge request from the feature branch
   ```bash
   git add -A
   git commit -m "update current report"
   git push
   ```

10. Report to the user.
   - Share the outcome, the validation result, and any remaining blockers.
