// The agents monitor: daemon health, the agent list with start/stop, recent
// outcomes, and a config reload. A separate top-level view (not over the RCA
// browser), mirroring the TUI's agents screen. Data + actions come from
// useAgents; presentation only here.

import type { JSX } from "react";

import { useAgents } from "../hooks/useAgents";
import type { AgentStatus } from "../types";

const CONFIG_PATH = "~/.config/beagle/agents.toml";

function AgentRow({
  agent,
  onToggle,
}: {
  agent: AgentStatus;
  onToggle: (id: string) => void;
}): JSX.Element {
  return (
    <li className="agent-row">
      <div className="agent-head">
        <span className="agent-id">{agent.id}</span>
        <span className={agent.running ? "agent-state running" : "agent-state paused"}>
          {agent.running ? "running" : "paused"}
        </span>
        {agent.active_sessions > 0 ? (
          <span className="agent-sessions">{agent.active_sessions} session(s)</span>
        ) : null}
        {agent.last_tick !== null ? (
          <span className="agent-tick">last tick {agent.last_tick}</span>
        ) : null}
        <button
          type="button"
          className="agent-toggle"
          onClick={() => {
            onToggle(agent.id);
          }}
        >
          {agent.running ? "Stop" : "Start"}
        </button>
      </div>
      {agent.last_results.length > 0 ? (
        <ul className="agent-results">
          {agent.last_results.map((result) => (
            <li key={result}>{result}</li>
          ))}
        </ul>
      ) : null}
    </li>
  );
}

export function AgentsView({ onBack }: { onBack: () => void }): JSX.Element {
  const { event, notice, start, stop, reload } = useAgents();
  const connected = event?.connected ?? false;
  const status = event?.status;

  return (
    <main className="app app-agents">
      <section className="agents">
        <header className="agents-header">
          <button type="button" className="agents-back" onClick={onBack}>
            ← RCAs
          </button>
          <h1 className="agents-title">Agents</h1>
          <span className={connected ? "agents-badge online" : "agents-badge offline"}>
            {connected ? `● beagle-agentd ${status?.version ?? ""}` : "● daemon offline"}
          </span>
          <button type="button" className="agents-reload" onClick={reload}>
            Reload config
          </button>
        </header>

        {!connected ? (
          <p className="agents-offline">
            {event?.error ?? "connecting to beagle-agentd…"} — start it with{" "}
            <code>beagle agent start</code>.
          </p>
        ) : null}

        {connected && status !== undefined ? (
          status.agents.length > 0 ? (
            <ul className="agents-list">
              {status.agents.map((agent) => (
                <AgentRow
                  key={agent.id}
                  agent={agent}
                  onToggle={(id) => {
                    if (agent.running) {
                      stop(id);
                    } else {
                      start(id);
                    }
                  }}
                />
              ))}
            </ul>
          ) : (
            <p className="agents-empty">
              No agents configured. Edit <code>{CONFIG_PATH}</code>, then Reload config.
            </p>
          )
        ) : null}

        <footer className="agents-footer">
          {notice !== null ? <span className="agents-notice">{notice}</span> : null}
          <span className="agents-hint">
            Edit <code>{CONFIG_PATH}</code> and press Reload config to apply changes.
          </span>
        </footer>
      </section>
    </main>
  );
}
