"use strict";
// Shared by the tests: the recorded outputs, a temporary folder, and the hook run as the process
// Claude Code starts.
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawn } = require("node:child_process");

const KIT = path.join(__dirname, "..");
const HOOK = path.join(KIT, "hooks", "record.js");

/** A recorded output, by file name: see fixtures/. */
const fixture = (name) => fs.readFileSync(path.join(__dirname, "fixtures", name), "utf8");

function tempFolder(t) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "tests-kit-"));
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  return dir;
}

/** The hook, as Claude Code starts it. Resolves { code, stdout, stderr, ms }. */
function runHook(stdin, env = {}, script = HOOK) {
  return new Promise((resolve, reject) => {
    const started = Date.now();
    const base = { ...process.env };
    for (const name of ["HANGAR_API_URL", "HANGAR_API_TOKEN", "HANGAR_PANE_ID", "HANGAR_PANE_NAME", "HANGAR_KITS_DATA", "CLAUDE_PROJECT_DIR", "CLAUDE_PLUGIN_ROOT"]) delete base[name];
    const child = spawn(process.execPath, [script], { env: { ...base, ...env }, stdio: ["pipe", "pipe", "pipe"], windowsHide: true });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => (stdout += chunk));
    child.stderr.on("data", (chunk) => (stderr += chunk));
    child.on("error", reject);
    child.on("close", (code) => resolve({ code, stdout, stderr, ms: Date.now() - started }));
    child.stdin.end(stdin);
  });
}

/** What Claude Code hands a PostToolUse hook for a call that succeeded. */
const successEvent = (command, stdout, extra = {}) =>
  JSON.stringify({
    session_id: "session-1",
    cwd: extra.cwd || "/work/project",
    hook_event_name: "PostToolUse",
    tool_name: extra.tool || "Bash",
    tool_input: { command },
    tool_response: { stdout, stderr: "", interrupted: false, isImage: false },
  });

/** And for one that exited non-zero, which is how a failing test run arrives. */
const failureEvent = (command, output, extra = {}) =>
  JSON.stringify({
    session_id: "session-1",
    cwd: extra.cwd || "/work/project",
    hook_event_name: "PostToolUseFailure",
    tool_name: extra.tool || "Bash",
    tool_input: { command },
    error: `Exit code ${extra.exitCode || 1}\n${output}`,
    is_interrupt: false,
  });

module.exports = { KIT, HOOK, fixture, tempFolder, runHook, successEvent, failureEvent };
