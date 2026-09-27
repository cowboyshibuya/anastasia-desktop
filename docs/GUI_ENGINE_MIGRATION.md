# Restore the original desktop interface

Status: in progress on `codex/restore-original-ui`. The original GPUI views are
copied and compile, but the old daemon transport has not been replaced. The
desktop launcher deliberately refuses to start it. Do not package or release
this branch until a private-home create/prompt/stream/resume cycle and visual
comparison pass against the original app.

The first engine client slice is now in `anastasia-client::engine_connection`.
It pins Rust SDK revision `9516895` and passes a private-home create, plan,
rename, message-persist, detach, and resume round trip against the matching
engine executable. The adapter now translates streaming tool, permission, and
question events for the original GUI. It does not yet connect the GPUI
application to the engine. The old client protocol remains only as unmigrated
source; the launcher still blocks it.

The desktop source in this branch comes from `../anastasia`. Preserve its GPUI
views, assets, keyboard behavior, window restoration, and settings screens.
The agent engine in `anastasia-engine` remains the only owner of sessions,
provider turns, tools, approvals, and transcript history.

## Boundary

Replace the prototype's `anastasia-client` transport and daemon supervisor
with an adapter over the versioned harness API. Keep GUI-only work (window
state, drafts, browser, terminal, file panels, themes, notifications) in the
desktop process. Store those preferences beneath `ANASTASIA_CLI_HOME/desktop`
or `~/.anastasia-cli/desktop`; never read the prototype's `~/.anastasia` data.

## Required routes

| Original UI behavior | Engine owner or route |
| --- | --- |
| Session list, create, attach, rename, archive | Harness session API |
| Prompt, steering, cancel, streamed transcript | Harness turn API |
| Planning, questions, permissions, model selection | Harness planning and control API |
| Transcript hydration and resume | Harness history API |
| Project, branch, worktree, review, file actions | GUI service over engine-scoped workspace APIs; extend the API where absent |
| Browser, local terminal, window state, themes, drafts | Desktop-owned GUI services |
| Usage, provider discovery, credentials | Engine API; add missing read/control routes |

Do not ship the copied UI while its old daemon launcher is still active. Remove
the old client protocol and provider driver paths after the harness adapter
passes a real create/prompt/stream/resume cycle.

## Blocking compatibility gaps

- The original UI uses UUID task/runtime identities; the engine API uses string
  session IDs. Persist one mapping and hydrate transcripts from engine history.
- The original client protocol includes task-state, draft, workspace, usage,
  skills, terminal, attachment, and computer-use commands. The engine API does
  not yet cover all of those. Route GUI-only services locally and add only
  agent-owned operations to the engine API.
- The copied driver and supervisor still speak the prototype WebSocket
  protocol. Replace them before enabling launch. Never run the prototype
  daemon to make this branch look functional.
