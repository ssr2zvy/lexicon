# Continuous implementation workflow

1. Fetch the latest remote state.
   ```bash
   git fetch
   ```

2. Ensure the current branch is `main`.
   - If the current branch is not `main`, switch to it.
   ```bash
   git switch main
   ```

3. Pull the latest changes for `main`.
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

7. Validate using the required build script.
   ```bash
   bash automation/build_bundle_install/build_bundle_install.sh
   ```
   - Do not run ad hoc `cargo` commands or custom installation flows during validation.
   - If the script succeeds, the latest Lexicon CLI will be available.
   - If the script fails, report the exact failing build or step.
   - If this flow does not lead to the intended result, stop and tell the user exactly what failed; do not pivot to a different strategy without explicit confirmation.

7. Replace `current.md` with your implementation report.
   - Write the implementation report based on the work completed in Steps 4 through 6.

8. Commit and push the updated report.
   ```bash
   git add -A
   git commit -m "update current report"
   git push
   ```

9. Report to the user.
   - Share the outcome, the validation result, and any remaining blockers.
