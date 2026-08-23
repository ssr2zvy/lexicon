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

4. Read `current.md` and plan the implementation.
   - Determine how you will complete the implementation request described in `current.md`.

5. Implement the required source changes.
   - Keep the implementation aligned with the plan in Step 4.
   - Do not pivot to a new strategy without clear confirmation from the user if the plan is not producing the intended result.

6. Validate using the required build script.
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
