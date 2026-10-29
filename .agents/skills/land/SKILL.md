---
name: land
description: >-
  Land explicitly requested changes using the current project's own tooling:
  verify formatting, review the code, and commit and merge locally. Run tests
  only when explicitly requested, and obtain separate confirmation before every
  push to origin. Invoke only when the user requests landing or merging,
  including an explicit /land invocation; never for review, preparation,
  passing checks or skill installation alone.
disable-model-invocation: true
metadata:
  delta-action: land
---

# Land changes locally

## Scope and authorization

This workflow is independent of programming language, framework and hosting
provider. Discover the current repository's conventions instead of assuming a
toolchain, a branch named `main` or a GitHub-based process.

An explicit landing request, including `/land`, authorizes preparing scoped
commits and completing the local merge below. Proceed without asking for that
same local-merge permission again. Creating or installing this skill is not a
landing request.

Every push to `origin` requires a separate, explicit confirmation after showing
the proposed publication. Invoking this skill, approving a local merge, passing
checks or confirming skill installation does not supply push permission.

## Inspect the project and preserve work

1. Read applicable `AGENT.md` and `AGENTS.md` files, contribution policies,
   relevant plans and workflow definitions. Respect conditional requirements for
   signing, reviews, authorship, changelogs and submission metadata. Do not invent
   requirements, weaken existing ones or repeat questions already answered.
2. Inspect the local repository root, branch, HEAD, index, staged/unstaged diffs,
   untracked files, remotes and relevant history. Identify the requested change
   and the intended local destination branch from the request and project rules.
   Ask a focused question if either remains ambiguous.
3. Work locally. Do not automatically fetch, publish a working branch, open a PR,
   start remote CI, install tools or change remote configuration. Local
   remote-tracking references may be stale; do not present them as live remote
   verification. If external information or a different checkout is necessary,
   explain the need and obtain the appropriate authorization before proceeding.
4. Preserve unrelated staged and unstaged edits. Never blindly stage the whole
   repository, automatically stash or reset work, amend unrelated commits, or
   rewrite shared history. If related changes cannot be isolated safely, stop
   and ask rather than disturbing other work.
5. Identify the project's actual formatter and verification commands from its
   manifests, task definitions, scripts or configuration. Verify their arguments
   and local runtime requirements; documentation alone may be stale. Record the
   repository-relative source for every project-specific command used. Do not
   substitute commands from a different language or guess a missing script.

## Prepare and integrate locally

- Include existing related uncommitted changes in preparation; do not assume
  that the user has already created a commit or working branch.
- Select explicit related paths or hunks. Check for accidentally included
  credentials, private data, unrelated files and generated artifacts. Follow the
  project's commit-message and signing conventions.
- Use a working branch when necessary to keep the candidate separate from the
  destination. Integrate the current local destination into that candidate
  without rewriting existing commits. Do not silently update another checkout.
- Resolve conflicts automatically only when the intended result is clear and
  unrelated work can be preserved. For ambiguous conflicts, stop, identify the
  affected files and decision, and ask the user. Do not pick ours/theirs
  wholesale or hide an unresolved conflict.
- Inspect the integrated result, including any merge changes, before the checks
  and review below. Keep required project documentation accurate without
  presenting unperformed checks as completed.

## Verify formatting

Run the project's defined formatter in check mode where available, using its
configured scope. If formatting needs correction, apply only related changes
with the configured formatter and verify the resulting candidate again.

If no formatter is defined, report that fact and inspect the diff for obvious
whitespace/style inconsistencies. Do not invent a formatter command, install a
formatter automatically, reformat unrelated code or claim an automated pass.

## Review the code

Perform a code review of the complete candidate diff against the intended local
destination, not just the most recently edited file. Read the surrounding code
and relevant callers/contracts where needed. Focus on:

- Correctness, regressions, error handling and integration behavior.
- Security, privacy, data loss and concurrency risks relevant to the change.
- Compliance with the request, project boundaries and existing conventions.
- Whether conflict resolutions preserve both intended behavior and unrelated
  work, and whether evidence supports the stated outcome.

Fix actionable findings within the authorized scope before merging. Pause for
ambiguous intent or fixes that require expanding the scope. Review the resulting
diff again after substantive fixes, and recheck affected formatting. Do not
equate a clean formatter result, compilation or the absence of tests with a
successful code review.

## Tests and other required checks

Run tests only when the user explicitly requests them for this landing or has
already given an applicable explicit test instruction. A bare landing request
does not request tests. Do not trigger tests indirectly through a broad build,
validation wrapper, pre-push hook or CI submission without checking what it runs.

If binding project rules require tests that the user has not requested, stop
and explain the required check, then ask for authorization to run it. Do not run
it silently or bypass the rule. Existing evidence is reusable only if it
demonstrably covers the exact candidate and satisfies the project's rules.

When tests are requested, use verified project commands and the appropriate
scope, report the actual results and rerun affected tests if later changes
invalidate them. Do not invent commands or turn a narrow request into unrelated
expensive suites.

Honor applicable mandatory non-test checks and reviews using local facilities.
If a required gate depends on unavailable tools, remote CI, external review or
other unapproved activity, stop and explain the blocker rather than silently
weakening the requirement. All checks required for the local merge must have
passed on the candidate being merged. Pending, failing, missing or unverifiable
required checks are not success.

State explicitly when tests, native UI checks, permissions or real external
effects were not verified. Do not claim unrequested tests passed.

## Complete the local merge

1. Commit the selected changes, including scoped fixes and required
   documentation, using a non-interactive commit command. Preserve unrelated
   index and working-tree content.
2. Before advancing the local destination, verify that its tip and the
   candidate still match the reviewed and checked state. If either changed,
   integrate the new state and repeat affected formatting, review and applicable
   checks. Apply the conflict preference above.
3. Merge into the intended local destination using the project's supported
   strategy. Prefer a fast-forward when it preserves the intended history and
   no project rule says otherwise. Use explicit non-interactive commit messages;
   prefix Git commands that can open an editor with `GIT_EDITOR=true`.
4. If the destination is checked out elsewhere, do not alter that checkout
   without authorization. Explain the blocker and ask how to proceed rather
   than silently claiming a working-branch commit completed the merge.
5. Verify the local destination actually contains the requested change, record
   the resulting commit IDs, and inspect the final index and working-tree
   status. Do not delete branches or files containing other work.

## Ask before publishing

After the local merge, show:

- The actual configured `origin` URL and the intended remote branch.
- The local commit IDs and a concise summary of what would be sent.
- Formatting, code-review and explicitly requested test results, plus known
  limitations or unmet publication requirements.

Then ask explicitly whether to push that described change to `origin`. Stop
and wait. Do not publish a working branch, create a PR, enable auto-merge or
invoke a script/hook that publishes before receiving that confirmation.

If the user declines, leave the merge local and report that nothing was pushed.
If `origin` is absent or its identity/destination is ambiguous, ask rather than
creating a remote or substituting a `local` backlink.

After confirmation, verify the destination and applicable publication rules
using only the authorized external operations. Push exactly the approved
commits and branch, never force-push, rewrite shared history, bypass protections
or silently include later changes. Do not disable required hooks to avoid their
tests; if a required hook would run unrequested tests, ask first.

If the remote advanced, reject a non-fast-forward push safely. Explain the
divergence, obtain permission for any needed fetch, integrate without rewriting
shared history, and repeat affected review and checks. Show the changed
publication and ask again before a new push.

If the destination requires PRs, remote checks or external reviews, explain the
requirements and obtain authorization for that expanded publication workflow.
Do not create them automatically or bypass their gates. A successful push may
trigger server-side CI independently; do not present that as a requested or
completed test run.

After an approved push, verify that the intended remote branch contains the
approved change. Report a publication failure or unverifiable result accurately,
including when a push succeeded but follow-up verification did not.

## Report the outcome

Distinguish clearly between:

- Prepared commits versus a completed local merge.
- Local merge versus an approved and verified push.
- Formatting results, code-review findings and tests actually requested/run.
- Known limitations, skipped checks and genuine blockers.

Report the actual local/remote destination and resulting commit IDs. If blocked
before the local merge, state that the change has not landed locally. If merged
locally but not published, say so explicitly; do not describe it as published.
Installing this skill is neither a local merge nor a push.
