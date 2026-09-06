# GitHub disaster-recovery backup

Status: implementation evidence for GitHub Issue [#309](https://github.com/nurockplayer/tachiko-work/issues/309). GitHub is the sole collaboration and authority surface. The GitLab project is a read-only, backup-only replica; do not use its Issues, merge requests, or settings as project authority.

The repository-owned path is the Free-compatible workflow
[`github-dr-backup.yml`](../../.github/workflows/github-dr-backup.yml) plus
[`scripts/github-dr-backup.sh`](../../scripts/github-dr-backup.sh). It does not
create a GitLab project, configure GitLab mirroring, or mutate GitHub.

## What runs and what is retained

The workflow checks out canonical GitHub `main` with full history and runs the
driver against one configured GitLab HTTPS target:

- `17 * * * *` UTC: source-only Git branch/tag replication, at least hourly.
- `43 2 * * *` UTC: source replication followed by a complete metadata snapshot, at least daily.
- `workflow_dispatch`: choose `full` (the default) or `source-only` for an operator-run check.

Runs are serialized with a non-canceling concurrency group. A complete run
captures repository identity/default branch, branches, tags, Issues and
comments, Pull Requests and conversation/review data, labels, milestones,
releases, and rulesets when the token can read them. Git workflow definitions
are already part of the replicated Git refs. The manifest records the schema,
source HEAD, UTC timestamp, object counts, explicit omissions, and SHA-256
payload checksums.

The production workflow pins the driver's retention to seven dated snapshots
(`GITHUB_DR_RETENTION_COUNT=7`). The CLI override is available only for a
bounded operator-run capture and is not exposed as a workflow input. The
workflow's `dr-metadata` branch keeps `snapshots/latest` plus the seven most
recent dated snapshot directories. The branch is independent from source-ref
refreshes, so pruning a stale source branch cannot remove the only metadata
history. A failed API call or incomplete capture fails the run before
publication and leaves the previous `latest` view intact. The `verify`
command must pass before a snapshot is committed.

Actions runtime logs, artifacts, caches, credentials, Actions secrets, and
other protected values are intentionally not captured. An unavailable
rulesets endpoint is disclosed in `omitted_or_unavailable`; another API or
transport failure remains a failed backup rather than an asserted success.

## One-time GitLab target setup

This is an operator/provider gate, not an action performed by the workflow.

1. Create one private, empty GitLab project on GitLab.com. A GitLab Free
   project is sufficient; native pull mirroring is not required. Use the
   project only as a backup repository and keep GitHub as the upstream.
2. Ensure the project has a `main` branch after the first source-only run (the
   workflow always addresses `main` explicitly). Do not enable bidirectional
   synchronization or use GitLab Issues/MRs as a coordination surface.
3. Add this repository variable, with no credentials embedded in its value:

   `DR_GITLAB_TARGET` — full HTTPS Git URL, for example
   `https://gitlab.com/<namespace>/<project>.git`.

4. Add this optional repository variable when the GitLab credential uses a
   non-default username:

   `DR_GITLAB_USERNAME` — normally `oauth2` for a GitLab Project Access
   Token. The workflow defaults to `oauth2` when it is empty.

5. Add this repository Actions secret:

   `DR_GITLAB_TOKEN` — a GitLab Project Access Token for this one
   project, with the minimum role that can push and the `write_repository`
   scope only. Do not grant `api`, `read_api`, group administration, or
   organization-wide scopes. Do not put the token in the target URL.

The workflow maps these legal GitHub Actions names to the driver's internal
`GITHUB_DR_GITLAB_TARGET`, `GITHUB_DR_GITLAB_USERNAME`, and
`GITHUB_DR_GITLAB_TOKEN` environment variables. GitHub repository variable
and secret names beginning with `GITHUB_` are reserved and must not be used
for this setup.

The workflow uses the ephemeral built-in `GITHUB_TOKEN` only through `GH_TOKEN`
for read-only GitHub API calls. Its declared permissions are `contents: read`,
`issues: read`, and `pull-requests: read`; no GitHub write permission is
requested. A transient `GIT_ASKPASS` file supplies the GitLab token to Git
transport and is created under the runner's temporary directory.
It is never committed, printed, or serialized into snapshot payloads.

## Manual activation and evidence

After the target and credentials are configured, run **GitHub DR backup → Run
workflow → `full`**. Do not treat a skipped, unauthorized, or unavailable
GitLab run as PASS. The final workflow line emits a machine-readable evidence
record; attach that record to Issue #309 only after independently checking the
target project. It must contain all of these fields:

| Field | Required observation |
| --- | --- |
| `target` | Exact GitLab project URL/project identity, without credentials |
| `source_head` | GitHub `refs/heads/main` SHA observed by the run |
| `mirrored_head` | GitLab `refs/heads/main` SHA, equal to `source_head` |
| `metadata_head` | GitLab `dr-metadata` commit after the snapshot commit |
| `snapshot_at` | UTC timestamp from `latest/manifest.json` |
| `snapshot_checksum` | SHA-256 of the captured manifest |
| `counts` | Manifest counts for each captured object class |

The first live activation remains an external gate until this evidence is
recorded. Local fixture output from the acceptance scripts is repository-side
evidence, not proof that a live GitLab target is configured.

## Verification and restore drill

The repository-side fixture checks can run without mutating either provider:

```sh
bash scripts/github-dr-backup-check.sh
bash scripts/github-dr-restore-check.sh
```

For an operational restore drill, use a credential manager or a transient
askpass environment; never place a token in a clone URL or shell trace:

```sh
git clone --branch main "$DR_GITLAB_TARGET" restored-source
git clone --branch dr-metadata "$DR_GITLAB_TARGET" restored-metadata
bash scripts/github-dr-backup.sh verify \
  --snapshot restored-metadata/snapshots/latest
```

Check the restored default branch, non-default source branches, tags, and the
metadata manifest/checksums. If GitHub is available, compare the observed
source HEAD before using the backup. Recovery is read-only with respect to
GitHub; any decision to recreate GitHub-side objects requires a human review
and must not turn GitLab into a second collaboration authority.

## Rotation and revocation

1. Create a replacement GitLab Project Access Token with the same project,
   role, and `write_repository`-only scope.
2. Update `DR_GITLAB_TOKEN` in repository Actions secrets. Run a manual
   `full` workflow and verify the mirrored HEAD, metadata commit, manifest
   checksum, and counts.
3. Revoke the old token after the replacement evidence is recorded. If a
   token may have been exposed, revoke it immediately, replace the secret,
   inspect the GitLab audit trail, and do not copy the exposed value into an
   Issue, PR, log, or snapshot.

The built-in GitHub token is ephemeral and managed by GitHub Actions. Recheck
the declared read permissions if the GitHub API adds a new captured class; a
permission denial must remain an explicit omission or a failed run, never an
unreported truncation.
