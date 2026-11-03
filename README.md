# Tests

A Hangar kit that shows whether the tests pass without anyone reading a terminal, and
hands a failure to an agent in one click.

## What it does

**A hook** (Claude Code `PostToolUse` and `PostToolUseFailure` on `Bash` and
`PowerShell`, plain Node, no dependency) notices a test run **by its command**: vitest,
jest, pytest, cargo test, go test, node --test, or a package script (`pnpm test`,
`npm test`, `yarn test`, `bun run test`), behind `npx`, `pnpm exec`, `python -m`,
`uv run`, an environment prefix, or in a chain (`cd app && pytest`). A command that only
mentions a runner (`git commit -m "fix vitest"`, `cat vitest.config.ts`) is not a run.

It reads the output of the run and keeps, for the last run of each agent in each project:

```json
{ "framework": "vitest", "passed": 12, "failed": 2, "skipped": 1, "duration": 2300,
  "failures": [{ "name": "math > adds", "file": "src/math.test.js", "line": 4, "message": "…" }] }
```

- **It reads what the framework itself says**: vitest, jest, pytest, cargo (the sum of
  every test binary), node:test (spec and TAP), go (plain, `-v` and `-json`). Colours,
  Windows line ends and progress bars do not matter.
- **An output it cannot read says « not understood », never « passed ».** No summary, no
  test run, a summary cut short, or a green summary under a non-zero exit code: none of
  those is a pass. An output cut before its summary still lists the failures it shows,
  marked as cut.
- **It never slows the agent and never gets in its way.** Anything but a test run costs
  one Node start. On any error of its own it says nothing and exits 0.

**A tab**, « Tests »: the last run per project and agent, a badge with the number of
failures, and each failure as a row with its place and its message. **Confier à un
agent** sends that failure to a Claude Code agent of the workspace (by default the one
that ran it), or **Confier les échecs** the first five at once. What is sent says what
failed, the command to run again and from where, and puts the failure's message between
markers, labelled as output copied from a test run: data to read, never instructions to
follow. Designed states: no run yet, all green, not understood, denied, and a send
refused.

**The tab is native** (Hangar 2.0 or later): Rust compiled to WebAssembly (`tabs/tests.wasm`,
built from `native/`), drawn with Hangar's own widgets. The manifest is schema 2 and the tab's
`entry` is the module; there is no HTML page.

**A skill** that tells agents to read the last recorded result, rather than run
everything again, when they are only asked about the state of the tests.

## What it can touch

- **The hook** reads the tool call it is handed (the command and its output) and writes
  into the kit's own data folder, `$HANGAR_KITS_DATA/tests/`: `index.json` and
  `runs/<key>.json`, the last 40 runs of the last three days. Nothing else: no file of the
  project, no network, no command run. The output is parsed and not stored; a failure's
  message is kept (2 000 characters at most, 50 failures at most per run), and so is the
  command line as typed.
- **The tab** holds `data.read` (to read those files), `panes.read` (to list the
  agents) and `panes.send` (to send the brief, when you click: never by itself). It writes
  nothing and runs nothing. It makes these calls: `data.read`, `panes.list`, `panes.send`, `tab.setTitle` and
  `tab.setBadge`; it also copies a command or a message to the clipboard and says « Copied » when you press the copy button
  (`ui.copy`, `ui.toast`, which need no permission).

It does not change the tests or their output, and sees no test run that did not go
through a Bash or PowerShell call of an agent.

## What it does not do

- A run in a terminal of yours, or in a pane without Claude Code, is not seen.
- `bun test` and runners it does not know are « not understood ».
- `go test` without `-v` counts packages, not tests: the number passed is left blank.
- A command whose output went to a file (`pnpm test > out.txt`) has nothing to read.
- **The tab cannot select text**: its widgets draw text that the pointer cannot select, so
  a command and a failure's message have a copy button. It shows the first five failures of a
  run and folds the rest behind « Show N more » (a view may not pass 4 MB, and 40 runs of 50
  failures would); « Hand over all » still sends the first five, as it always did.

## Get it

Link `kits/tests` from Hangar, or clone
<https://github.com/hangar-ai/kit-tests>. Needs Hangar 2.0.0 or later (the tab is native; an
older Hangar says the kit needs a newer one).
Its tests: `node --test kits/tests/`, and `cargo test --manifest-path native/Cargo.toml`.

## This repository

```text
hangar-kit.json        the manifest: schema 2, the tab's `entry` is its module
tabs/tests.wasm        the native tab, built and committed
native/                its source: a Rust crate on hangar-kit-sdk (cdylib and rlib)
  src/logic.rs         the hook's records, turned into what the tab shows: pure
  src/strings.rs       its English and French
  src/lib.rs           the tab: state, messages, view
  tests/               the tab's tests, over hangar_kit_sdk::testing::Harness
hooks/, skills/        the hook and the skill
scripts/build-native.sh
```

`cargo test --manifest-path native/Cargo.toml` runs the tab's tests, on the host, with no
WebAssembly. `scripts/build-native.sh` builds `tabs/tests.wasm` (`rustup target add
wasm32-unknown-unknown` once; the Rust version is in `native/rust-version`). Hangar runs the
module a kit ships and never builds one, so the module is committed in the same commit as the
source that made it; `scripts/build-native.sh --check`, which CI runs, fails if it is not what
`native/` builds. The module is the same bytes wherever it is built: the paths of the machine are
remapped away. It imports `hangar.log` and `hangar.now` and nothing else, so it needs no WASI.

`node --test` runs the tests of the hook (detection, parsers, what it records) and of the manifest
(schema 2, the module's imports and exports, the plugin, the skill), with Node alone. Nothing in
them needs Hangar's repository.

CI calls [kit-check](https://github.com/hangar-ai/.github/blob/rust-rewrite/.github/workflows/kit-check.yml),
and checks the native tab: formatting and lints, its tests, and that the committed module is what
`native/` builds. A release is a tag, `v` and the manifest's version, and it reaches Hangar's
gallery once [the catalog](https://github.com/hangar-ai/catalog) is updated and reviewed.
