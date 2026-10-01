---
name: gh
description: Use the GitHub CLI when work needs GitHub-hosted repository, issue, pull request, project, Actions, release, or API context, or when it must create, update, query, or verify those resources.
---

# GitHub CLI

Use `gh` as the primary interface to GitHub from this repository. Prefer its typed commands for common workflows and `gh api` when the typed surface does not expose the required operation.

## Establish context

Before a repository-scoped command:

1. Check authentication with `gh auth status` when the session may be unauthenticated or the operation will mutate GitHub state.
2. Resolve the repository explicitly with `gh repo view --json nameWithOwner --jq .nameWithOwner` when the remote, fork, or working directory could be ambiguous.
3. Pass `--repo OWNER/REPO` to commands that support it when the target is not unambiguous. This prevents acting on a fork or the wrong checkout.
4. Read the command's local help with `gh <command> <subcommand> --help` when flags, preview status, or output shape matter.

A failed command is evidence to inspect, not a reason to guess. Read its error, check the relevant help, and retry with the smallest correction.

Context is established when the target repository, authentication state, and command shape are known before the first repository-scoped operation.

## Choose the narrowest command

Use the highest-level command that owns the resource:

- `gh issue` for issues, comments, labels, and milestones.
- `gh pr` for pull requests, reviews, checks, comments, and merges.
- `gh repo` for repository metadata, cloning, forking, and settings exposed by the CLI.
- `gh project` for Projects.
- `gh run` and `gh workflow` for Actions runs and workflows.
- `gh release` for releases and assets.
- `gh search` for cross-repository search.
- `gh status` for a compact view of assigned and relevant work.

Use `gh browse` only when a human needs the browser. Use `gh api` for endpoints or fields unavailable through a typed command, and document the endpoint's purpose in the command or surrounding note.

Command selection is complete when the narrowest typed command is chosen, or the API endpoint and response shape are explicit.

## Query and format data

For automation, use structured output instead of terminal prose:

```sh
gh pr list --repo OWNER/REPO --state open --json number,title,headRefName,author

gh issue list --repo OWNER/REPO --json number,title,labels --jq '.[] | {number, title, labels: [.labels[].name]}'
gh run list --repo OWNER/REPO --json databaseId,status,conclusion,workflowName
```

Use `--jq` for small projections and filtering. Use `--template` for human-readable summaries. Keep the selected fields minimal. If a result can span pages, use the command's `--limit` or `gh api --paginate` deliberately and account for an empty result.

For `gh api`:

```sh
gh api repos/OWNER/REPO/pulls/123 --jq '{title, state, merged_at}'
gh api --paginate repos/OWNER/REPO/issues --method GET --slurp
```

GitHub API paths are relative to the API root unless an absolute URL is required. Use `-f` for string form fields, `-F` for typed fields, and `-H` for headers. For nested API input, pass separate hierarchical fields rather than relying on object-valued `-f` arguments.

Query preparation is complete when the output shape, pagination behavior, and empty-result behavior are accounted for.

## Mutate deliberately

Before any mutation, identify the exact target and show the intended effect. Prefer a dry run, preview, or read-back when the command supports one. Treat these as consequential operations:

- closing, reopening, labeling, assigning, or editing issues;
- posting comments or reviews;
- approving, merging, or closing pull requests;
- deleting branches, releases, assets, or other resources;
- changing repository settings, secrets, variables, permissions, or Actions state.

Use explicit identifiers and `--repo` for mutations. After a successful mutation, fetch the resource or relevant event and verify the resulting state. A successful exit code alone is not enough when the operation affects a workflow or review state.

Never print, paste, or commit tokens, cookies, private keys, or secret values. Let `gh` use its configured credential store and GitHub environment variables. When a command would expose secret material, query metadata such as names, timestamps, or status instead of values.

Mutation is complete only after the intended target and effect are identified, the command succeeds, and a follow-up read verifies the resulting state.

## Common workflows

### Issues and discussions

```sh
gh issue list --repo OWNER/REPO --state open --label LABEL --limit 100
gh issue view NUMBER --repo OWNER/REPO --comments
gh issue create --repo OWNER/REPO --title "TITLE" --body-file PATH
gh issue comment NUMBER --repo OWNER/REPO --body-file PATH
gh issue edit NUMBER --repo OWNER/REPO --add-label LABEL --add-assignee USER
```

Use `gh issue view` before editing or closing so the current body, labels, assignees, and comments are part of the working context.

### Pull requests

```sh
gh pr list --repo OWNER/REPO --state open --json number,title,headRefName,isDraft,reviewDecision

gh pr view NUMBER --repo OWNER/REPO --comments
gh pr checks NUMBER --repo OWNER/REPO

gh pr diff NUMBER --repo OWNER/REPO
```

For merge decisions, inspect checks, review state, mergeability, and branch status before using `gh pr merge`. Prefer the repository's configured merge method and avoid bypass flags unless explicitly required.

### Actions

```sh
gh run list --repo OWNER/REPO --workflow WORKFLOW --limit 20
gh run view RUN_ID --repo OWNER/REPO --log-failed
gh run rerun RUN_ID --repo OWNER/REPO --failed
```

After rerunning or dispatching a workflow, capture the new run identifier and inspect its status rather than assuming the operation fixed the problem.

## Completion check

A GitHub operation is complete when the command exited successfully, the intended repository and resource were unambiguous, and any mutation or asynchronous operation has been verified with a follow-up read. Report the target, the command's result, and any remaining authentication, permission, or workflow uncertainty.
