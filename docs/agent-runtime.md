# The beagle agent runtime

An always-on, native-Rust daemon that turns **RCAs you mark ready into
remediation PRs** — and the TUI/desktop surfaces to watch and steer it. It is
the in-repo, Rust-native successor to the Go `ai-pipelines/beagle` feature
agent; the two coexist today (see [Coexistence](#coexistence-with-the-go-pipeline)).

```
RCA at status `agent`  ──▶  beagle-agentd  ──▶  isolated git worktree
                                                     │
                                       headless `claude -p` implements
                                       remediation.md and commits
                                                     │
                              Rust pushes → opens a PR → beagle pr add
                                       → advances the RCA to final-review
```

Deterministic Rust owns every risky step (git, GitHub, RCA status); `claude`
only edits code. A run is isolated in its own worktree and OS process group, is
supervised (a panic or failure never takes the daemon down), retries under a
cap, and cleans up after itself — a crash never leaks a worktree or a
token-burning session.

## Layout

```text
crates/beagle-agent/    the engine: config, sqlite job store, RCA poller,
                        git-worktree manager, headless claude runner, the
                        orchestrator, and the control protocol + client/server
crates/beagle-agentd/   the always-on daemon that hosts the engine and serves
                        the control socket
cli/  (beagle)          `beagle agent …` — install/uninstall/start/stop/status,
                        and the TUI's Agents screen
desktop/                the desktop app's Agents view (Tauri commands + a live
                        event stream)
```

## Quick start

```sh
# 1. Build the daemon and put it on PATH (or set BEAGLE_AGENTD to its path).
cargo build --release -p beagle-agentd
cp target/release/beagle-agentd /usr/local/bin/

# 2. Configure at least one agent.
mkdir -p ~/.config/beagle
$EDITOR ~/.config/beagle/agents.toml        # see the format below

# 3. Autostart it (launchd on macOS, systemd --user on Linux).
beagle agent install

# 4. Watch it: press `A` in the beagle TUI, or click "Agents" in the desktop app.
beagle agent status                          # or a one-shot from the shell
```

Then move an RCA to `status = "agent"` (`beagle status <id> agent`) and the
daemon picks it up on its next poll.

## Configuring agents (`agents.toml`)

The config lives at `~/.config/beagle/agents.toml` (or
`$XDG_CONFIG_HOME/beagle/agents.toml`). It is optional — an absent file means
"no agents", and the daemon idles. Unknown keys are rejected, so typos surface
immediately. One `[[agent]]` table per agent:

```toml
[[agent]]
# Stable id, unique in the file. Used as the job key and shown in the UI.
id = "rca-remediation"

# Whether the daemon runs this agent. Defaults to true.
enabled = true

# What makes an RCA actionable. v1: an RCA sitting at this lifecycle status.
trigger = { kind = "rca-status", status = "agent" }

# The git repository the agent implements changes in. `~/` is expanded.
target_repo = "~/GitHub/matthewmyrick/beagle"

# A custom prompt file handed to the headless claude session (see below).
prompt = "~/.config/beagle/prompts/rca-remediation.txt"

# The --allowedTools list for the session. Empty inherits the runner default.
allowed_tools = ["Read", "Edit", "Bash(git:*)", "Bash(gh:*)", "Bash(cargo:*)"]

# How often the trigger is polled. Accepts ms / s / m / h. Defaults to 60s.
poll_interval = "60s"

# The most concurrent sessions this agent may run. Defaults to 2.
max_concurrent = 2
```

Edit it any time. From the UIs, editing and reloading is one step (the TUI's
`e`, the desktop's **Reload config**); by hand, run `beagle agent status` after
a restart, or send a reload from a UI. Enable/disable and pause/resume changes
apply live; adding or removing an `[[agent]]` needs a daemon restart.

### Custom prompts

`prompt` points at a plain-text file — the base instructions for the session.
The daemon appends the RCA's id, title, and `remediation.md` beneath it, then
runs `claude -p` in the isolated worktree. Keep the prompt focused on *how* to
implement a fix in `target_repo` (house rules, verification commands); the
per-RCA specifics come from `remediation.md`.

## Autostart & control: `beagle agent`

```sh
beagle agent install      # write + load the service (starts on login, restarts on crash)
beagle agent uninstall    # unload + remove it
beagle agent start        # (re)start the loaded service
beagle agent stop         # stop it (stays stopped until start/install)
beagle agent status       # query the running daemon over its control socket
```

- **macOS** installs a launchd `LaunchAgent` (`~/Library/LaunchAgents/com.beagle.agentd.plist`,
  `RunAtLoad` + `KeepAlive`).
- **Linux** installs a `systemd --user` unit
  (`~/.config/systemd/user/beagle-agentd.service`, `Restart=always`,
  `WantedBy=default.target`). To keep it running without an active login:
  `loginctl enable-linger $USER`.

`beagle-agentd` also runs directly: `beagle-agentd` loops; `beagle-agentd once`
runs a single tick per agent and exits (handy for a manual end-to-end).

## Watching agents

Both UIs have a dedicated **Agents screen**, separate from the RCA browser, that
never disturbs it:

- **TUI** — press `A` (`A`/`Esc` to go back). `j`/`k` select an agent, `x`
  starts/stops it, `Enter` opens its latest session log, `e` edits `agents.toml`
  and reloads. It shows daemon health, each agent's running/paused state, active
  sessions, last tick, and recent outcomes, auto-refreshing every ~2s.
- **Desktop** — click **Agents** in the sidebar (**← RCAs** to go back).
  Per-agent **Start/Stop**, a **Reload config** button, and the same live
  status, streamed over Tauri events.

A down daemon shows a clean offline banner in both — it never hangs the UI.

## How it works (contracts)

- **Control socket** — a loopback unix socket
  (`$XDG_RUNTIME_DIR/beagle-agentd.sock`, else
  `~/.local/state/beagle/agentd.sock`). Never a network port. It speaks
  newline-delimited JSON: `get-status`, `list-agents`, `start-agent`,
  `stop-agent`, `reload-config`, and `subscribe` (a live push stream). The TUI
  and desktop are thin clients over it.
- **State** — under `$XDG_STATE_HOME/beagle-agent` (else
  `~/.local/state/beagle-agent`): `jobs-<id>.db` (SQLite job state,
  `pending → processing → done/failed`, reset on boot), `logs/<agent>/<rca>.log`
  (per-session output), `runs/` (live-session registry for orphan cleanup),
  `worktrees/<agent>/<rca>` (isolated checkouts).
- **Auth** — the daemon is passive: it never triggers a login. It idle-skips a
  tick unless `gh auth status` and `claude --version` both succeed.
- **RCA store root** — resolved from `$BEAGLE_AGENT_RCA_ROOT`, else the daemon's
  working directory.

## Coexistence with the Go pipeline

The Go `ai-pipelines/beagle` feature agent (GitHub issue → merged PR) and this
runtime (RCA → remediation PR) do related but distinct jobs, and **run side by
side today**. This runtime is the intended successor. The switch-over happens
once it has proven, in daily use, that it:

1. drives real RCAs to merged remediation PRs without manual cleanup,
2. matches the Go pipeline's crash-safety and orphan-handling in practice, and
3. covers the triggers you actually rely on.

Until then, run whichever fits the task; nothing here retires the Go pipeline.
