---
name: land
description: >-
  Land the change in the current Delta worktree of the EBG repository
  (eholk/ebg): commit it, publish a feature branch and pull request against
  main, wait for the required "build" check, merge with a merge commit, and
  verify the result on origin/main. Invoke only when the user has explicitly
  requested landing, merging, or shipping the change, such as choosing Land
  Changes or running /land. Do not invoke it to review or prepare a change, to
  run checks, or to install this skill.
metadata:
  delta-action: land
---

# Land changes in eholk/ebg

An explicit landing request (Land Changes, `/land`, "land this", "merge this")
*is* approval to commit, push, open the PR, and merge. Once invoked, carry the
workflow through to the merge. Do not re-ask whether to merge, and do not stop
at "the branch is pushed, the PR is open". Stop only for the blockers in
[Stop and ask](#stop-and-ask).

## Verified project facts

Confirm these still hold before relying on them (`gh api
repos/eholk/ebg/branches/main/protection`, `git log --oneline --graph -12`):

- `main` is protected with one required status check, context `build`, and
  `strict: true` (the branch must be up to date with `main` before merging).
  Required signatures are disabled and no PR reviews are required.
- Work lands through pull requests merged with a **true merge commit**
  (`Merge pull request #NNN from ...`), preserving the branch's commits. See
  `1c1abe7` (PR #259) and `e910e9f` (PR #258). Squash and rebase merges are not
  used here even though the repository allows them.
- Commit subjects are free-form imperative sentences with no conventional-commit
  prefix ("Add default wayback link decorations").
- PR bodies are short summaries; the linked issue is closed with `Closes #NNN`
  or `Fixes #NNN` (see PR #259's "Closes #21").
- There is no `CONTRIBUTING` file, no PR template, and no changelog requirement
  for a change: `release-plz` maintains `CHANGELOG.md` from `main`.
- `gh` is authenticated as `eholk`; `origin` is `git@github.com:eholk/ebg.git`
  and pushes work over SSH. `local` is the user's primary checkout: never push to
  it.

## Workflow

### 1. Confirm request and scope

The workspace is a Delta worktree of a dedicated clone; the repository root is
the current directory.

```bash
git --no-optional-locks status --short
git log --oneline origin/main..HEAD
git diff --stat origin/main
```

Establish what is being landed. If the worktree holds unrelated work you cannot
separate from the change, stop and ask. Never commit secrets or credentials.
If there is genuinely nothing to land, say so and stop.

### 2. Commit the change

Stage only the intended paths (`git add <paths>`), or `git add -A` when the
worktree contains nothing else. Commit with an imperative subject matching the
history:

```bash
GIT_EDITOR=true git commit -m "Deduplicate external links per post"
```

Skip this step when the change is already committed. Confirm the tree is clean
before continuing.

### 3. Create the feature branch

```bash
git fetch origin
git switch -c <descriptive-kebab-case>
```

Branch names in this repository are short and descriptive (`wayback`,
`better-docker-generation`, `new-post-date`). If `HEAD` is already a feature
branch that is not on `origin`, push that branch instead of creating a new one.
Never commit or push to `main` directly.

### 4. Verify locally

These are the exact CI commands, from `.github/workflows/rust.yml` (job `build`):

```bash
cargo build --locked
cargo test --locked
```

Also run the docs build when the change touches the generator, renderer,
templates, or `doc/` (the final CI step, `cargo run -- build doc` in
`.github/workflows/rust.yml`; the `build` subcommand and its positional path are
defined in `src/main.rs` and `src/cli/build.rs`):

```bash
cargo run -- build doc
```

A representative local run is a sanity check, not a substitute for the required
remote check. Fix failures before publishing; do not push a branch you know
fails.

### 5. Publish and open the PR

```bash
git push -u origin HEAD

body=$(mktemp)
cat > "$body" <<'EOF'
<one-paragraph summary of the change and why>

Closes #NNN
EOF
gh pr create --base main --title "<imperative title>" --body-file "$body"
```

Drop the `Closes` line when the change is not tied to an issue. Note the PR
number `gh pr create` prints.

### 6. Wait for the required check

Required checks must pass **on the commit being landed**, not an earlier one:

```bash
gh pr checks <NNN> --required --watch --fail-fast
gh pr view <NNN> --json headRefOid,statusCheckRollup
```

Every required check must report success for the current `headRefOid`. Pending,
failing, missing, or unverifiable checks are not success; report failure instead
of merging.

Because protection is strict, the branch must also be current with `main`:

```bash
git fetch origin
git merge origin/main
```

Merging (not rebasing) `main` into the branch matches existing history
("Merge branch 'main' into wayback"). Re-run the checks after every push.

**Conflicts**: resolve them automatically when the intended result is clear,
preserving unrelated work, then re-run local verification and push. If a
resolution is genuinely ambiguous or would drop work you did not author, stop
and ask rather than guessing.

### 7. Merge

```bash
gh pr merge <NNN> --merge
```

`--merge` produces the true merge commit this repository uses. Do not use
`--squash` or `--rebase`, and do not push to `main` directly. Do not delete the
remote branch (`delete_branch_on_merge` is false; deleting is not part of this
workflow).

### 8. Verify the destination

```bash
gh pr view <NNN> --json state,mergedAt,mergeCommit
git fetch origin && git log --oneline origin/main -1
```

Confirm the PR state is `MERGED`, its `mergeCommit` is the tip of `origin/main`,
and the linked issue closed if one was referenced. Do not claim success from a
passing build, a pushed branch, or an open PR alone.

### 9. Report the outcome

When running in a subthread and `report_subthread_status` is available, use it;
otherwise report in the conversation. Keep `title` to a few sentence-case words
and `description` to one short line, linking the landed commit and the CI run:

```bash
gh run list --workflow rust.yml --branch <branch> --limit 1 \
  --json databaseId,url,conclusion,headSha
```

Commit links use `https://github.com/eholk/ebg/commit/<sha>`. Report
`status: "success"` only after step 8 confirms the change is on `origin/main`.
Report `status: "failure"` for a failed attempt or blocker, naming it (for
example "Blocked by CI" with the run URL, or "Merge conflicts"). A failure is
not terminal: continue any safe recovery that is still permitted, then report
the updated outcome. Ignore `release-plz` (runs on push to `main` and opens a
release PR) and Docker release jobs; neither is required for this landing.

## Stop and ask

- A required check fails or never appears, and you cannot make it pass.
- A conflict cannot be resolved confidently, or resolution would discard work.
- Push, PR, or merge permission is denied (for example, a protected-branch
  rejection you cannot satisfy).
- The change's scope is unclear, or unrelated work cannot be separated from it.
- There is nothing to land.
