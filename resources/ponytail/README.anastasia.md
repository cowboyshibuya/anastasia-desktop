# Vendored Ponytail

This directory is a **verbatim, partial copy** of upstream Ponytail. Anastasia ships
it so that Ponytail is available offline, at a known version, without downloading or
executing a moving `main` at session start.

| | |
|---|---|
| Upstream | https://github.com/DietrichGebert/ponytail |
| Version | 4.8.4 |
| Pinned revision | `bc9ee949d5f439e8b9f3bb92c6d6d3d1e6ebd324` (tag `v4.8.4`) |
| License | MIT — see [LICENSE](LICENSE), Copyright (c) 2026 DietrichGebert |

## No upstream file was modified

Every file here except this README is byte-identical to the pinned revision.
Anastasia adapts Ponytail through the interfaces upstream already exposes — the
`PONYTAIL_DEFAULT_MODE` environment variable and the `PLUGIN_DATA` state directory —
never by patching it. Keep it that way: an Anastasia-specific fork of Ponytail would
have to be re-merged on every upstream release.

## The one file Anastasia adds

`package.json` is not from upstream. Upstream's hooks are CommonJS and rely on
their repository root having no `"type"` field; vendored into Anastasia they would
instead inherit the repository root's `"type": "module"` and fail to load with
`require is not defined in ES module scope`. This file pins `"type": "commonjs"`
for the vendored tree so the hooks resolve the same way they do upstream. It does
not shadow `pi-extension/package.json`, which declares its own `"type": "module"`.

## What is included, and why

| Path | Why Anastasia needs it |
|---|---|
| `.claude-plugin/plugin.json` | Makes this directory loadable by `claude --plugin-dir`, and is the single source of truth for the version string Anastasia reports. |
| `hooks/` | The lifecycle hooks Claude Code runs: session activation, subagent propagation, and `/ponytail <mode>` tracking. |
| `skills/ponytail/SKILL.md` | The ruleset itself. Anastasia reads this file directly to build the instruction text for runtimes that have no hook mechanism. |
| `pi-extension/` | Ponytail's native Pi extension, loaded with `pi --extension`. |

Upstream material Anastasia does not use is deliberately omitted: benchmarks, docs,
translated READMEs, the statusline scripts, and the adapters for hosts Anastasia does
not drive (Cursor, Windsurf, Cline, Kiro, Gemini, Copilot, OpenClaw).

## Upgrading

Copy the same paths from a newer upstream tag and update the version and revision in
the table above. `crates/anastasia-core/src/ponytail.rs` reads the version out of
`.claude-plugin/plugin.json`, so there is no version string to update in Rust.
