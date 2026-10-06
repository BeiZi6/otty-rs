// Otty integration plugin for OpenCode (ESM) — the real logic.
//
// Reports agent state changes to the Otty app over IPC so a terminal pane can
// show the processing / idle / awaiting-input badge. This file ships as a
// readable, code-signed module inside Otty.app so you can audit exactly what
// runs. Otty installs a tiny shim at
//   ~/.config/opencode/plugins/otty-integration.js
// that imports this module's default export and calls it with the per-install
// values (plugin id, otty-cli path, IPC socket):
//
//   import createOttyPlugin from "file:///…/otty-plugin.js";
//   export default createOttyPlugin({ id, cli, socket });
//
// The object that produces is readable by both of OpenCode's plugin loaders,
// which it runs side by side over the same plugins directory:
//   * V1 (OpenCode ≥ 1.3.4) takes a default export `{ id, server }` and calls
//     `server(input)` for the hooks below. It ignores named exports once it
//     sees that shape.
//   * V2 requires a default export `{ id, setup }` (or `{ id, effect }`); a
//     module without one fails to load. Its plugin context has no session
//     events or tool hooks yet, so `setup` has nothing to subscribe to — the
//     state reporting all goes through the V1 `server` hooks.
//
// OpenCode ≤ 1.3.3 calls every export as a plugin function, so it cannot load
// an object default export. The V2 loader rejects a function default export,
// so no single shim works on both.
import { spawn } from "child_process";

function sessionIdFrom(event) {
  return event?.properties?.sessionID
    || event?.properties?.info?.id
    || event?.sessionID
    || event?.sessionId
    || event?.session?.id
    || event?.properties?.session?.id
    || "";
}

function directoryFrom(event, fallbackDirectory, fallbackWorktree) {
  return event?.properties?.info?.directory
    || event?.properties?.directory
    || event?.directory
    || fallbackDirectory
    || fallbackWorktree
    || "";
}

function stateFromStatus(status) {
  const type = status?.type || status || "";
  if (type === "busy" || type === "retry") return "processing";
  if (type === "idle") return "idle";
  return null;
}

async function startOttyPlugin({ client, directory, worktree }, { cli, socket }) {
  const CLI = cli || "otty-cli";
  const SOCKET = socket || "";

  // Notifications must reach Otty in EMISSION order. At end of turn opencode
  // emits a trailing `session.status:{busy}` in the same tick as `session.idle`
  // (busy → idle → idle). Each notify used to spawn an independent detached
  // otty-cli and fire-and-forget it, so those processes connected to the Otty
  // socket in a nondeterministic order — the stale `processing` could land
  // AFTER the final `idle`, and since Otty applies agent state in arrival order
  // (last write wins), the tab badge stuck spinning forever. Chaining each
  // spawn behind the previous one (await its exit before starting the next)
  // makes delivery order match emission order, so the terminal `idle` is always
  // applied last. Even with two plugin shims installed (each its own chain),
  // every chain ends on `idle`, so a `processing` is never the last write.
  let sendChain = Promise.resolve();

  function spawnCli(args, env, onError) {
    return new Promise((resolve) => {
      let settled = false;
      let timer = null;
      const done = () => {
        if (settled) return;
        settled = true;
        if (timer) clearTimeout(timer);
        resolve();
      };
      let proc;
      try {
        proc = spawn(CLI, args, { stdio: "ignore", detached: true, env });
      } catch (error) {
        if (onError) void onError(error);
        return done();
      }
      proc.on("error", (error) => {
        if (onError) void onError(error);
        done();
      });
      // otty-cli round-trips the IPC request/response, so `close` means Otty
      // has already applied this state — the safe point to release the next
      // send in the chain.
      proc.on("close", done);
      // Never let a hung otty-cli stall the chain (and thus the final idle).
      // `done` clears this on the normal close/error path so the watchdog
      // doesn't linger (and kill an already-exited pid) after completion.
      timer = setTimeout(() => {
        try { proc.kill(); } catch {}
        done();
      }, 3000);
      if (timer.unref) timer.unref();
      proc.unref();
    });
  }

  function notify(sessionId, state, cwd, onError) {
    if (!sessionId) return;
    const args = [
      "state:opencode",
      `session-id=${sessionId}`,
      `state=${state}`,
      // process.pid is the opencode process; Otty matches it against each
      // pane's process tree (pid-accurate), and correctly ignores opencode
      // instances running outside Otty (e.g. an editor).
      `agent-pid=${process.pid}`,
    ];
    if (cwd) {
      args.push(`cwd=${cwd}`);
    }

    const env = { ...process.env };
    if (SOCKET) env.OTTY_SOCKET = SOCKET;
    sendChain = sendChain.then(() => spawnCli(args, env, onError)).catch(() => {});
  }

  async function log(level, message, extra = {}) {
    try {
      await client.app.log({
        body: {
          service: "otty-plugin",
          level,
          message,
          extra,
        },
      });
    } catch {}
  }

  async function dispatch(sessionId, state, cwd, message, extra = {}) {
    if (!sessionId) return;
    await log("info", message, {
      sessionId,
      state,
      cwd,
      ...extra,
    });
    notify(sessionId, state, cwd, async (error) => {
      await log("error", "Failed to spawn otty-cli", {
        sessionId,
        state,
        cwd,
        cli: CLI,
        socket: SOCKET,
        error: String(error),
      });
    });
  }

  await log("info", "OpenCode plugin initialized", {
    directory: directory || "",
    worktree: worktree || "",
    cli: CLI,
    socket: SOCKET,
  });

  // OpenCode has no dedicated "completed" event — a session is a state machine
  // and "busy → idle" IS task completion. The authoritative idle signals are
  // `session.idle` and `session.status` with {type:"idle"}; busy/retry mean
  // processing. `session.updated` is deliberately NOT here: its payload is the
  // whole Session object, so it fires on every metadata change (title, cost,
  // tokens, time) DURING active work and again as trailing flushes AFTER idle.
  // Treating it as idle made Otty see repeated processing→idle edges within a
  // single prompt — one spurious "Task complete" notification per tool/metadata
  // cycle (5+ per prompt) plus a flickering tab badge. Registration of the
  // session id is covered by chat.message / session.created / the app's HTTP-log
  // fallback, so dropping it loses no functionality.
  const interestingTypes = new Set([
    "session.created",
    "session.status",
    "session.idle",
    "tui.session.select",
  ]);

  return {
    event: async ({ event }) => {
      const type = event?.type || "";
      const sid = sessionIdFrom(event);
      const cwd = directoryFrom(event, directory, worktree);
      if (!interestingTypes.has(type)) return;

      if (!sid) {
        await log("warn", "OpenCode event missing session ID", {
          type,
          cwd,
        });
        return;
      }

      if (type === "session.created") {
        // A freshly created session is genuinely idle and has no preceding
        // work, so this never fires a "Task complete" notification; it just
        // seeds the session id early.
        await dispatch(sid, "idle", cwd, "Dispatching session lifecycle event", {
          type,
        });
        return;
      }

      if (type === "tui.session.select") {
        await dispatch(sid, "idle", cwd, "Dispatching TUI session selection", {
          type,
        });
        return;
      }

      if (type === "session.status") {
        const state = stateFromStatus(event?.properties?.status || event?.status);
        if (state) {
          await dispatch(sid, state, cwd, "Dispatching session status", {
            type,
            status: event?.properties?.status?.type || event?.properties?.status || event?.status?.type || event?.status || "",
          });
        } else {
          await log("warn", "OpenCode status event did not map to Otty state", {
            type,
            sessionId: sid,
            cwd,
            status: event?.properties?.status?.type || event?.properties?.status || event?.status?.type || event?.status || "",
          });
        }
        return;
      }

      if (type === "session.idle") {
        await dispatch(sid, "idle", cwd, "Dispatching session idle", {
          type,
        });
      }
    },
    "chat.message": async (input) => {
      const sid = input?.sessionID || "";
      const cwd = directory || worktree || "";
      if (!sid) {
        await log("warn", "chat.message missing session ID", {
          cwd,
        });
        return;
      }
      await dispatch(sid, "processing", cwd, "Dispatching chat message", {
        type: "chat.message",
        agent: input?.agent || "",
        model: input?.model?.providerID && input?.model?.modelID
          ? `${input.model.providerID}/${input.model.modelID}`
          : "",
        variant: input?.variant || "",
      });
    },
    "command.execute.before": async (input) => {
      const sid = input?.sessionID || "";
      const cwd = directory || worktree || "";
      if (!sid) {
        await log("warn", "command.execute.before missing session ID", {
          cwd,
          command: input?.command || "",
        });
        return;
      }
      await dispatch(sid, "processing", cwd, "Dispatching command execution", {
        type: "command.execute.before",
        command: input?.command || "",
      });
    },
    "tool.execute.before": async (input) => {
      const sid = input?.sessionID || "";
      const cwd = directory || worktree || "";
      if (!sid) {
        await log("warn", "tool.execute.before missing session ID", {
          cwd,
          tool: input?.tool || "",
        });
        return;
      }
      await dispatch(sid, "processing", cwd, "Dispatching tool execution", {
        type: "tool.execute.before",
        tool: input?.tool || "",
        callId: input?.callID || "",
      });
    },
    "permission.ask": async (input) => {
      const sid = input?.sessionID || "";
      if (!sid) {
        await log("warn", "permission.ask missing session ID", {
          cwd: directory || worktree || "",
        });
        return;
      }
      await dispatch(sid, "awaiting", directory || worktree || "", "Dispatching permission ask", {
        type: "permission.ask",
      });
    },
  };
}

// The plugin definition a current (v4) shim default-exports. `id` differs per
// build ("otty-integration" / "otty-integration-dev") so a release and a dev
// install can both be loaded without one replacing the other in V2.
export default function createOttyPlugin({ id, cli, socket }) {
  return {
    id,
    server: (input) => startOttyPlugin(input, { cli, socket }),
    setup() {},
  };
}

// For v3 shims still on disk (they are only replaced when the user accepts the
// "Update OpenCode Plugin" prompt). A v3 shim sets the paths on globalThis and
// `export *`s this module, which carries named exports only, so this must stay
// the module's only named export: OpenCode's V1 loader calls every one of them
// as a plugin. The globals are read at invocation time, after the shim's
// assignments have run. Every shim sets the same globals, so with a release
// and a dev v3 shim both installed, both report to whichever loaded last.
export const OttyPlugin = (input) => startOttyPlugin(input, {
  cli: globalThis.__OTTY_CLI,
  socket: globalThis.__OTTY_SOCKET,
});
