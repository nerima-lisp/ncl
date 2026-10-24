# Release process

## Version policy

NCL uses `0.y.z` versions until maintainers record a declaration that the command-line and language interfaces are stable in the [project decisions](src/project/wave-plan.md#decisions). Increase `y` for a feature release or an incompatible change. Increase `z` for fixes that preserve documented behavior. After `1.0.0`, use the usual Semantic Versioning rules: major for incompatible changes, minor for compatible features, and patch for compatible fixes.

Keep the version in the root `[workspace.package]` section of `Cargo.toml` and the package version in `flake.nix` equal. Tag releases as `v<version>`. Never move or reuse a tag after pushing it.

## Release decision

Do not tag a release until all of these conditions hold for the same `main` commit:

- Every job in the latest run of [CI](../.github/workflows/ci.yml) for that commit succeeds, and the `scoreboard` job in the latest [Conformance scoreboard](../.github/workflows/scoreboard.yml) run succeeds for that commit. Record the commit SHA with both run IDs and keep the scoreboard JSON artifact from `conformance/scoreboard/` as evidence.
- The CI coverage report meets its 95% region threshold. The local command and threshold are documented in [development.md](src/project/development.md#rust-coverage).
- A Markdown `cl-bench` table contains results for the 64 measured cases, excluding `WALK-LIST/MESS`, with three NCL and three SBCL measurements per case from the same day and machine. Run the paired baseline in [Conformance data](../conformance/README.md#paired-optimization-baseline), then use its three NCL `default` runs and three SBCL runs to fill a table with the benchmark name and six times in seconds. Keep the raw JSON report beside the table in `conformance/scoreboard/`, and include the date and commit SHA in both filenames. The project performance targets are documented in [wave-plan.md](src/project/wave-plan.md#project-acceptance-criteria); its 65-case threshold remains unresolved because SBCL disables `WALK-LIST/MESS`.
- A per-test `ansi-test` inclusion report records the result for each test in the same-day SBCL baseline. Save the Markdown comparison under `conformance/scoreboard/` with the date and commit SHA in its filename. The current Linux scoreboard marks ANSI as `unmeasured`, so a green scoreboard job alone does not satisfy this condition; use the same-day baseline comparison described in [Conformance data](../conformance/README.md#ansi-test-inclusion-comparison).

The ANSI comparison must complete, and every test passed by the matching SBCL baseline must pass under NCL. A failure, timeout, harness error, or unexecuted test blocks the release. Keep the release on hold until maintainers resolve the disabled benchmark in D-11 and update the corresponding [project acceptance criterion](src/project/wave-plan.md#project-acceptance-criteria) consistently.

## Prepare and verify a release

1. Choose a version under the policy above. Update the root workspace version in `Cargo.toml` and the package version in `flake.nix`.
2. Move the completed entries from `## [Unreleased]` in `CHANGELOG.md` into a dated `## [x.y.z] - YYYY-MM-DD` section. Add a fresh `## [Unreleased]` section above it. Keep the notes focused on user-visible changes.
3. Run the repository's CI and review the Conformance scoreboard, coverage, `cl-bench`, and `ansi-test` evidence against the release decision criteria. Redirect the ANSI comparison's Markdown output to its evidence file. Its exit status 1 means at least one SBCL-passing test failed, timed out, encountered a harness error, or remained unexecuted; inspect the report to identify each result. Confirm that all evidence refers to the same `main` commit.
4. Merge the version and changelog update to `main`. Wait for every CI job to succeed on the resulting `main` commit and repeat the release decision check.
5. Fetch the latest `main`. If it has advanced since the commit checked in step 4, repeat the release decision check for the new commit and wait for its CI jobs to succeed. Then confirm the version and changelog section, and create and push a `v<version>` tag at that exact `main` revision:

   ```sh
   jj git fetch --remote origin --branch main
   jj tag set vX.Y.Z -r main@origin
   jj git push --remote origin --tag vX.Y.Z
   ```

   Replace `X.Y.Z` with the version in `Cargo.toml`. The workflow fails if the tag does not match that version or the changelog has no matching section.
6. Open the `Release` workflow run for the tag. Confirm that all four target jobs pass their release build, workspace tests, CLI smoke checks, and checksum verification. Confirm that the GitHub Release contains all four target archives and their `.sha256` files, and that its notes match the corresponding changelog section.

## Dry-run a candidate

The `workflow_dispatch` event runs the release build, workspace tests, and CLI smoke checks on all four D-12 targets, then uploads one archive and checksum per target without creating a tag or GitHub Release. The targets are `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`, and `aarch64-apple-darwin`. GitHub requires the workflow file to exist on the default branch before it can be dispatched, and the candidate branch must already be pushed to GitHub. Authenticate `gh` with an account allowed to dispatch workflows and download their artifacts, then confirm the workflow and branch prerequisites:

```sh
gh auth status
gh workflow run release.yml --ref <candidate-branch>
gh run list --workflow release.yml --branch <candidate-branch> --limit 5
gh run view <run-id>
```

Select the run for the candidate commit, wait for all four build jobs to succeed, and download all four artifacts using that run ID. Verify each artifact's checksums:

```sh
gh run download <run-id> --name ncl-x86_64-unknown-linux-gnu --dir artifacts/linux-x64
gh run download <run-id> --name ncl-aarch64-unknown-linux-gnu --dir artifacts/linux-arm64
gh run download <run-id> --name ncl-x86_64-apple-darwin --dir artifacts/macos-x64
gh run download <run-id> --name ncl-aarch64-apple-darwin --dir artifacts/macos-arm64
(cd artifacts/linux-x64 && shasum -a 256 -c ./*.sha256)
(cd artifacts/linux-arm64 && shasum -a 256 -c ./*.sha256)
(cd artifacts/macos-x64 && shasum -a 256 -c ./*.sha256)
(cd artifacts/macos-arm64 && shasum -a 256 -c ./*.sha256)
```

## Cancel or recover from a failed release

- If a dry-run fails, read the failed job log, fix the source or workflow in a new candidate, then dispatch it again.
- If a tag-triggered run fails before publishing, cancel any remaining jobs and inspect the workflow run and tag. Do not move or reuse the tag. Fix the cause on `main`, complete the release checks, and use a new version and tag.
- [GitHub CLI's `release create` command](https://cli.github.com/manual/gh_release_create) creates a draft while it uploads assets, then publishes it. If an upload or publication fails and the draft remains, inspect it with `gh release view vX.Y.Z`. Delete the draft with `gh release delete vX.Y.Z` if it should not be published; leave the tag in place and use a new version after fixing the cause.
- If an invalid release was published, do not move its tag. Withdraw the release if repository policy permits, document the correction, and publish the fix under a new version after all release checks pass.
