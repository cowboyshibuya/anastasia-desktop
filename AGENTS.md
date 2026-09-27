# Anastasia Desktop

- The agent runtime, sessions, tools, approval policy, and provider credentials
  belong to `cowboyshibuya/anastasia-engine`. Use its versioned harness API;
  never add a second daemon or provider driver here.
- This app reads engine sessions from `~/.anastasia-cli`. Do not read, modify,
  or migrate the prototype GUI's `~/.anastasia` data.
- Keep GPUI rendering free of filesystem, network, subprocess, and blocking
  waits. Own async tasks and subscriptions; batch streaming updates.
- Preserve the multiline composer contract and keyboard access to every
  control. Test against a private `ANASTASIA_CLI_RUNTIME_DIR` and home.
- Pin the engine API dependency and bundled engine executable to the same
  revision. A desktop release must verify the bundled executable, and macOS
  must sign it with the rest of the app bundle.
- Keep the original Waku attribution and GPL license for reused GUI code.
