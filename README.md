# Anastasia Desktop

Anastasia Desktop is a native GPUI client for the [Anastasia agent engine](https://github.com/cowboyshibuya/anastasia-engine). The engine owns sessions, turns, tools, planning, questions, approval policy, and provider state. This repository owns the desktop window and interaction layer.

The first release supports engine sessions, transcript streaming, a multiline composer, plan mode, selectable questions, approval questions, model selection, and permission levels. It does not import the older GUI's `~/.anastasia` data. The CLI and desktop see the same engine sessions under `~/.anastasia-cli`.

## Development

Build the engine binary from a sibling `anastasia-engine` checkout, then run:

```sh
ANASTASIA_ENGINE_BIN=../anastasia-engine/target/debug/anastasia cargo run
```

Use a private `ANASTASIA_CLI_HOME` and `ANASTASIA_CLI_RUNTIME_DIR` for isolated tests. The desktop connects to a compatible API bridge if one is already running, or starts the configured engine binary. It does not stop a shared engine on exit.

The GPUI dependency is locked to the revision in `Cargo.lock`. On macOS, building its Metal shaders requires the Xcode Metal Toolchain.

For a bundle containing matching desktop and engine binaries, run
`scripts/package.sh` with the engine checkout in `../anastasia-engine` (or set
`ANASTASIA_ENGINE_SOURCE`). The script checks that the engine source matches
the revision pinned in `Cargo.toml`.

## Attribution and license

This application contains GUI code from the original Anastasia prototype, a GPL-3.0-only fork of [Waku](https://github.com/egoist/waku). See [LICENSE](LICENSE) and [source origin](docs/ORIGIN.md). The agent engine retains its own upstream MIT attribution in its repository.
