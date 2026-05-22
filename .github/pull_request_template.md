<!--
Thanks for the contribution! Quick checklist below — please fill it in
so reviewers can ship your change fast. Delete sections that don't
apply.
-->

## What this changes

<!-- One paragraph: what the user-visible difference is, or what
internal property changes. Skip preamble. -->

## Why

<!-- The problem this solves. If linked to an issue, "Closes #123". -->

## How

<!-- The technical approach. Mention any alternatives you ruled out
and why. -->

## Screenshots / clips

<!-- Required for visible UI changes. A short capture is worth a
thousand words. -->

## Checklist

- [ ] `cargo fmt --check` passes
- [ ] `cargo clippy --all-targets -- -D warnings` passes
- [ ] `pnpm check` (svelte-check) passes
- [ ] `CHANGELOG.md` updated under `## [Unreleased]` for user-visible changes
- [ ] No new TODO/FIXME comments (or there's a tracking issue if there are)
- [ ] The PR is focused — refactors live in separate commits from behaviour changes where practical
