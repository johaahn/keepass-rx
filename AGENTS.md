# Agent rules

## Checks

Run `./check.sh fast` before committing; the pre-commit hook runs the same
command. `./check.sh full` adds every crate, the QML checks and the tests,
and CI runs it on every push. `./check.sh compile` checks only that the
changed code builds and is formatted. The user enables the hooks once per
clone with `git config core.hooksPath .githooks`.

The checks come from the `robots/` submodule, set up by `robots.toml`.
`robots/AGENTS.md` explains them.

A finding counts only when the change introduced it: on a line the change
adds or edits, in a new file, in a function the change made longer, or in
a file the change took over the size limit. Old findings are listed in the
summary and do not fail the check. Paying down old debt is a separate,
planned change.

## Files only the user changes

Agents must not edit `robots/`, `robots.toml`, `.gitmodules`, `check.sh`,
`.githooks/`, `clippy.toml`, `.claude/`, `.opencode/`, this file, or the
`[lints]` tables in any `Cargo.toml`. Do not disable hooks, use
`--no-verify` or `git commit -n`, change `core.hooksPath`, or set
`ROBOTS_GUARD_EDIT`.

Do not vendor or patch third-party dependencies, and do not add Cargo
patch, replacement or source overrides. Do not add `allow` or `expect`
attributes to silence a lint; fix the code the lint reports.

## Commits

A commit subject is one line of at most 72 characters, with no body and no
trailers.

<!-- robots:skills -->
## Which skill to load

| Editing | Skill |
|---|---|
| Sailfish QML and its .js files in keepass-rx/qml-sfos/ | silica |
| Ubuntu Touch QML and its .js files in keepass-rx/qml/ | lomiri-qml |
| Rust fields and types that QML sees, in keepass-rx/src/gui/ | rust-qmetaobject |
| A Sailfish RPM build or Sailfish SDK tests, or files in rpm/, sfos/ | sailfish-build |
| A click build, or files in clickable.yaml | clickable |
<!-- /robots:skills -->
