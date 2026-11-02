# Kaeri

Folder guides: `crates/CLAUDE.md`, `apple/CLAUDE.md`, `android/CLAUDE.md`, `website/CLAUDE.md`;
`.claude/rules/` adds plugin and store checks when those files are touched. Any app UI work follows
`docs/native-ux.md`, the behaviour spec for every native client. Procedures are skills:
`local-checks`, `merge-train`, `ci-triage`, `app-shots`.

## Checks

- One test: `cargo test -p <crate> <test_name>`. One integration file:
  `cargo test -p <crate> --test <file_stem>`. CI runs `cargo nextest run --workspace`.
- Lint and format: `cargo clippy -p <crate> --all-targets -- -D warnings`,
  `cargo fmt --all -- --check`. Platform builds and the HOME guard: the `local-checks` skill.
- Live probes under `examples/` can use real credentials, playback or tracker
  writes; run them only when the task calls for that integration. `KAERI_TEST_STORE` opts
  the store journey test into a real endpoint. Report skipped coverage and checks you could not run.

## Easy to miss

- `kaeri-store` is local watch history. The plugin store's server logic is
  `kaeri-store-core`, with native and WASM adapters.
- Provider plugins under `plugins/` are separate builds outside the workspace. This repository
  carries no plugins apart from the two references and two test fixtures.
- `website/` uses Bun; the JS reference plugin uses npm and its `package-lock.json`.

## Rules that always apply

- The legal line is "Kaeri doesn't host any shows. You add your own sources." The long form
  is on the website's legal page. In the apps it shows only in About. Copy stays neutral and
  legal-first: Kaeri never supplies, promotes or curates sources (`i18n/lint.toml` lists the
  words it never uses).
- No real show, site, fansub or release-group names in code, fixtures, tests or commits. Use
  made-up titles and `.example` hosts. CC-licensed media only for playback checks.
- Copy and docs: plain sentences and headings, no em dashes, no "honestly", "deliberately" or
  "concretely", no bold-label-then-dash list items, no asides about past bugs.
- Never print secrets: the release keystore and its passwords (`~/.kaeri-signing`,
  `~/.gradle/gradle.properties`), the Mac update key, tracker client secrets in config.
- Tests never run against the real HOME or `~/Library/Application Support/kaeri`. Use the
  HOME guard from the `local-checks` skill. Never launch the real app id `app.kaeri.app`, and
  never post real notifications on the Mac; use the Fixtures app and fakes.
- Commits are SSH-signed. "agent refused operation" means retry; never bypass signing. Commit
  with the real HOME, not inside the HOME guard (it hides `~/.gitconfig`). Messages end with
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`; PR bodies end with
  `🤖 Generated with [Claude Code](https://claude.com/claude-code)`.
- Strings people read come from `i18n/` (Fluent). Never edit generated strings; change the
  catalogue and run `cargo run -p i18n-tool -- generate`. See `docs/translations.md`.
- Don't push to main or create tags. PRs go against main only.

## How changes ship

- Work on a branch in your own worktree from `origin/main`; don't edit the primary checkout
  unless asked. Use your own cargo target dir and delete it when done.
- Ready branches are merged into a merge train (one PR, one CI run); see `merge-train`. Every
  train waits for the maintainer to try local Mac and Android builds of it before it merges.
- main requires the `CI passed` check. `gh pr merge --auto --merge` only after confirming that
  protection; never `gh pr merge` without `--auto`.
- Every push to main publishes a beta by itself, a GitHub pre-release, except pushes that only
  touch `website/**`, `docs/**`, top-level `*.md`, `**/CLAUDE.md` or `.claude/**`. After a merge,
  confirm the beta pre-release published (the Release run succeeded and the pre-release has all
  its files).
- The merge train sets the version in `Cargo.toml` and `Cargo.lock`, committed as
  "Release X.Y.Z" before it merges; no bot pushes to main. While the major is 0, a breaking
  change raises the minor (plugins need rebuilding, the previous version can't read the config
  or data, the store protocol breaks, or an OS version is dropped) and everything else raises
  the patch, features included. The major goes to 1 only when the maintainer asks.
  `packaging/next-version.sh` and a PR check enforce a newer version.
- Stable comes from the Promote workflow, weekly on Tuesday at 09:00 Copenhagen time or by
  dispatch for a fix release. The maintainer approves every promotion in the `stable-release`
  environment. Never dispatch Promote unless the maintainer says to in so many words. Promote
  checks itself; look at it only when asked or when it failed.
- Always compile and run the touched area's tests locally before pushing (`local-checks`).

## Keeping token use down

- Search logs and test output for the failing lines (`grep -E 'error|FAIL|panicked'`, `tail`);
  never read a whole CI log, build log or test transcript. See `ci-triage`.
- Don't open generated or bulky files unless the task is about them: goldens, recorded scenes,
  `deploy/**/store_bg.wasm`, `crates/kaeri-store-core/assets/admin/`, shaders, `.shots/`,
  `android/.engine/`, `apple/.engine/`. Diff them with git instead.
- Reports back to an orchestrator: about 150 words, outcome first. Put long detail (tables,
  measurements, logs) in a file in the scratchpad and give its path.
- Wait for your own long commands in the foreground (or an until-loop), rather than ending your
  turn while background work runs; each hand-back wakes the orchestrator.
- Disk fills fast with several builds. Keep it above 15 GB (`df -h /`); delete your own build
  output first.

# Compact instructions

Keep: the task and its acceptance criteria, branch and worktree names, what was changed and
committed, check results, open decisions. Drop: full logs, file listings, superseded plans.
