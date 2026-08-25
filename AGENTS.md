# Working agreement — endpointgen

The operating contract for **any** coding agent working in this repository. This file is
the single source of truth for the rules: Codex, Cursor and Gemini CLI read `AGENTS.md`
natively, and Claude Code loads it through the `@AGENTS.md` import in
[`CLAUDE.md`](CLAUDE.md). **Never fork these rules into a per-vendor file.**

**Rust binary** (`endpoint-gen`).

## Invariants (don't break these)

- **No Python.** Not a script, not `python3 -c`, not a heredoc. Reaching for it is the
  tell that a step is being solved by parsing when the tool that owns the answer could
  just be asked. Do not swap it for another parser either, and do not assume `jq` is
  present: it does not ship with macOS. A fixed-shape field is one `sed -nE` line;
  anything needing real parsing belongs in this repo's own language, where it can be
  tested. If a task seems to need Python, the approach is wrong.

- **This crate is one node in a coupled chain.** endpoint-libs, endpoint-gen,
  honey_id-types, endpoint-validator and the six backends break each other silently —
  not in the repo you edited, but later, elsewhere, with an error that does not name the
  cause. Before calling a change done, run the chain check:

  ```bash
  path/to/endpoint-libs/scripts/check-chain.sh
  ```

  The script lives in the endpoint-libs checkout and skips repos you do not have.
  Background and the traps that have actually bitten: [endpoint-libs `docs/chain.md`](https://github.com/pathscale/endpoint-libs/blob/main/docs/chain.md)
  and [`docs/release-order.md`](https://github.com/pathscale/endpoint-libs/blob/main/docs/release-order.md).

- **Keep `cargo fmt` and `cargo clippy --all-targets` clean.** Lint failures are part of the build here, not advisory.
- **Publishing to crates.io is irreversible.** A version number can never be reused, and yanking does not delete. Run `cargo publish --dry-run` first, publish from the merged default branch, and tag the release.
- **A pre-release version (`-alpha`, `-beta`) needs an exact dependency pin.** A plain `"2.0"` requirement will not match `2.0.0-alpha.1`, so consumers must be bumped deliberately.
- **Docs describe what is true now.** If you change behaviour, update the README and any affected doc in the same change.

## Build & test

```bash
cargo build
cargo test
cargo fmt && cargo clippy --all-targets    # run after every change
```

## Verification

Run what you build before reporting it done. Type-checks and tests verify code correctness,
not feature correctness — **if you can't run it, say so explicitly** rather than implying
success.

- Compare against the base branch rather than asserting: a pre-existing failing test or lint
  error is not something you introduced, and saying so requires checking.
- A build that finishes suspiciously fast was cached, not rebuilt. Force a real rebuild when
  the rebuild is the thing you're verifying.

## PR discipline

**Always paste the full PR URL** (`https://github.com/pathscale/endpointgen/pull/<n>`), not just the number, so it's
clickable.

<!-- DORMANT — CI-green gating. Do not follow this rule yet; re-enable it as its own project.

Why it's off: CI here does not reliably attach checks to pull requests, so
`statusCheckRollup` comes back empty and "wait for green" would teach an agent to wait on
nothing. Verify per repo before switching this on.

To enable: ensure the workflow runs on `pull_request:`, confirm checks attach to a PR, then
uncomment the rule below.

    After any push or PR, **check CI and don't call it done until it's green**:

    ```bash
    gh pr view <number> --repo pathscale/endpointgen --json statusCheckRollup
    ```

    CI running → wait and recheck. CI failed → read the logs, fix, push, wait for green.
-->

## Keeping docs honest

Hit a factual error here — a stale path, a wrong command, a moved status? Fix it in the same
change. Don't open cosmetic rewording PRs.

Learned something durable — a gotcha, a decision, a constraint? It belongs **in this repo's
docs**, not in your agent's private memory. Repo docs are versioned, reviewable, and visible
to every agent and human; private memory dies with your machine.

## Git workflow

- **Always specify the branch when pushing**: `git push origin branch-name`
- **Branch naming**: `fix/issue-description` or `feat/issue-description`
- **Force-push your own branch freely.** Rebasing a feature branch onto a moved
  base, or amending before review, is normal and correct — use
  `--force-with-lease` so you don't clobber someone else's push.
- **Never force-push the default branch.** That is the history everyone else builds on,
  and it is protected server-side for a reason.
- **Never create merge commits — this is a hard ban.** Not locally, not to refresh a
  branch, not to land a pull request. If your branch has fallen behind, **rebase** it onto
  the moved base (`git rebase origin/master`, then `--force-with-lease`). `git merge master`
  into a feature branch is not an acceptable shortcut: it adds a commit whose only content
  is the fact that you were behind, and it turns a readable line of work into a diamond.
- **Rebase is the default everywhere** — refreshing a branch, and landing a pull request.
  Individual commits carry information: what was tried, in what order, and why. A rebase
  keeps that granularity on the base branch, so write commits worth keeping and land them
  intact.
- **Landing a pull request means rebase, then fast-forward.** `git rebase origin/master`
  on the branch, then `git merge --ff-only <branch>` on the base, then push. Those two
  commands are the whole job, so don't reach for `gh pr merge`: its default writes a
  merge commit. Rebasing rewrites the commit SHAs, so GitHub cannot always detect that
  a branch landed — close such pull requests explicitly and say why.
- **Don't delete remote branches by hand.** Once the work is on the default branch it is
  reaped automatically. Deleting your own local copy is fine.
- **Squash is acceptable** where it genuinely makes things easier or is the more
  appropriate shape for the branch — one logical change scattered across fixup commits, or
  a long branch whose intermediate states aren't worth preserving. It is a judgement call,
  not a violation. Merging is the only thing that is never allowed.
- **Delete what is deprecated.** A superseded file, flag, branch or code path gets removed
  in the change that supersedes it, not left behind with a deprecation note.

## Guardrails

[`.claude/settings.json`](.claude/settings.json) and [`.claude/hooks/`](.claude/hooks/) make
Claude Code prompt a human before prod-affecting or destructive commands — pushes, publishing
to a registry, `gh pr merge`, cloud CLIs, recursive deletes, deploy scripts.

**Other agents don't get that net automatically.** Apply the same rule yourself: ask before
running any command family listed in
[`.claude/hooks/ask-before-risky-commands.sh`](.claude/hooks/ask-before-risky-commands.sh).
It is one layer of defence, not a guarantee — a pattern match over a command string is
best-effort.

## No AI attribution

Never add AI attribution to anything in this repo or leaving it: no "Generated with
Claude Code" / robot-emoji footers, no `Co-Authored-By: Claude` (or any AI) trailers,
and no AI credit in commit messages, PR or issue titles/bodies, changelogs, release
notes, or code comments. Applies to every agent and every vendor. Work product should
be indistinguishable from a human teammate's.
