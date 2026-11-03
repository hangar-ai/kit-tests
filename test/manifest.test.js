"use strict";
// The manifest and the kit's shape, checked by hand against docs/kits.md: Hangar's
// validator is Rust (src-tauri/src/kits/manifest.rs) and cannot be reached from a Node
// test. Each rule below is one of that page's.
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { KIT } = require("./helpers.js");
const { hangarFile, HANGAR_FILES_MISSING } = require("./hangar-ui.js");

const manifest = JSON.parse(fs.readFileSync(path.join(KIT, "hangar-kit.json"), "utf8"));
const read = (...parts) => fs.readFileSync(path.join(KIT, ...parts), "utf8");

function allFiles(dir = KIT, found = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    if ([".git", ".hangar-ui", "node_modules"].includes(entry.name)) continue;
    if (entry.name === "target" && path.basename(dir) === "native") continue; // cargo's build folder
    if (entry.isDirectory()) allFiles(path.join(dir, entry.name), found);
    else found.push(path.join(dir, entry.name));
  }
  return found;
}

function safePath(relative, extensions, at) {
  assert.equal(typeof relative, "string", at);
  assert.doesNotMatch(relative, /^([/\\]|[A-Za-z]:)|:|(^|[/\\])\.\.([/\\]|$)/, `${at}: must stay inside the kit`);
  assert.ok(extensions.some((ext) => relative.toLowerCase().endsWith(ext)), `${at}: ${extensions.join("/")}`);
  assert.ok(fs.existsSync(path.join(KIT, relative)), `${at}: ${relative} must exist`);
}

test("the manifest's own fields", () => {
  assert.equal(manifest.schema, 1);
  assert.equal(manifest.id, "tests");
  assert.match(manifest.id, /^[a-z0-9-]{2,40}$/);
  assert.ok(manifest.name.trim());
  assert.match(manifest.version, /^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/);
  assert.ok(manifest.description.trim());
  assert.equal(manifest.homepage, "https://github.com/hangar-ai/kit-tests");
  safePath(manifest.icon, [".svg", ".png"], "icon");
  assert.equal(manifest.commands, undefined, "it runs no declared command");
  assert.equal(manifest.setup, undefined, "and asks for no setup");
  assert.equal(manifest.hangar, ">=1.9.0");
});

test("the gallery's card: one sentence, and its tags", () => {
  const { permissionsSummary, tags } = manifest.gallery;
  assert.ok(permissionsSummary.trim());
  assert.match(permissionsSummary, /hook/);
  assert.match(permissionsSummary, /tab/);
  assert.ok(Array.isArray(tags) && tags.length > 0 && tags.every((tag) => typeof tag === "string" && tag.trim()));
});

test("the plugin is found at the root, named like the kit", () => {
  assert.equal(manifest.plugin, undefined);
  const plugin = JSON.parse(read(".claude-plugin", "plugin.json"));
  assert.equal(plugin.name, manifest.id);
  assert.equal(plugin.version, manifest.version);
  assert.match(read("skills", "tests", "SKILL.md").replace(/\r\n/g, "\n"), /^---\nname: tests\n/);
});

test("the tab: « Tests », and exactly the three permissions it needs", () => {
  assert.equal(manifest.tabs.length, 1);
  const [tab] = manifest.tabs;
  assert.equal(tab.id, "tests");
  assert.equal(tab.title, "Tests");
  safePath(tab.entry, [".html"], "tabs[0].entry");
  safePath(tab.native, [".wasm"], "tabs[0].native");
  safePath(tab.icon, [".svg", ".png"], "tabs[0].icon");
  assert.deepEqual(tab.permissions, ["data.read", "panes.read", "panes.send"]);
  assert.equal(tab.params, undefined);
});

test("the native tab: a module beside the page, which stays for a Hangar that opens pages", () => {
  const [tab] = manifest.tabs;
  assert.equal(manifest.schema, 1, "schema 2 is for a kit with no page to fall back to");
  assert.equal(tab.entry, "tabs/tests/index.html", "the page a Hangar 1.x opens");
  assert.equal(tab.native, "tabs/tests.wasm");
  assert.ok(!path.isAbsolute(tab.native) && !tab.native.split(/[/\\]/).includes(".."), "inside the kit");
  const file = path.join(KIT, tab.native);
  assert.ok(fs.statSync(file).size <= 64 * 1024 * 1024, "64 MB at most");
  assert.ok(!path.basename(file).startsWith("."), "not hidden");
});

test("the module is WebAssembly built for Hangar: its exports, and no import but hangar.log and hangar.now", () => {
  const bytes = fs.readFileSync(path.join(KIT, manifest.tabs[0].native));
  assert.deepEqual([...bytes.subarray(0, 4)], [0, 0x61, 0x73, 0x6d], "WebAssembly");
  const compiled = new WebAssembly.Module(bytes);
  const imports = WebAssembly.Module.imports(compiled).map((item) => `${item.module}.${item.name}`);
  assert.deepEqual(imports.sort(), ["hangar.log", "hangar.now"], "no WASI, no wasm-bindgen");
  const exports = WebAssembly.Module.exports(compiled).map((item) => item.name);
  for (const name of ["memory", "hangar_abi_version", "hangar_alloc", "hangar_init", "hangar_update", "hangar_view"]) {
    assert.ok(exports.includes(name), name);
  }
  // Built with the paths of the machine remapped: the same bytes anywhere, and none of a home folder.
  assert.doesNotMatch(bytes.toString("latin1"), /\/home\/|\/Users\/|[A-Za-z]:\\Users\\/);
});

test("the module's source is in the repository: a crate on the SDK, a lock, a script, and the build ignored", () => {
  const cargo = read("native", "Cargo.toml");
  assert.match(cargo, /crate-type = \["cdylib", "rlib"\]/);
  assert.match(cargo, /hangar-kit-sdk = \{ git = "https:\/\/github\.com\/hangar-ai\/hangar"/);
  assert.ok(fs.existsSync(path.join(KIT, "native", "Cargo.lock")), "the lock pins the SDK's commit");
  assert.ok(fs.existsSync(path.join(KIT, "native", "src", "lib.rs")));
  assert.match(read("scripts", "build-native.sh"), /wasm32-unknown-unknown/);
  assert.match(read("scripts", "build-native.sh"), /tabs\/tests\.wasm/);
  assert.match(read(".gitignore"), /^native\/target\/$/m);
  assert.match(read(".gitattributes"), /^\*\.wasm binary$/m);
});

test("the hooks: PostToolUse and PostToolUseFailure, on Bash and PowerShell, with a timeout", () => {
  const { hooks } = JSON.parse(read("hooks", "hooks.json"));
  assert.deepEqual(Object.keys(hooks).sort(), ["PostToolUse", "PostToolUseFailure"]);
  for (const [event, groups] of Object.entries(hooks)) {
    assert.equal(groups.length, 1, event);
    assert.equal(groups[0].matcher, "Bash|PowerShell", event);
    assert.equal(groups[0].hooks.length, 1);
    const [hook] = groups[0].hooks;
    assert.equal(hook.type, "command");
    assert.equal(hook.command, 'node "${CLAUDE_PLUGIN_ROOT}/hooks/record.js"');
    assert.ok(hook.timeout > 0 && hook.timeout <= 10);
    assert.ok(fs.existsSync(path.join(KIT, "hooks", "record.js")));
  }
});

test("the hook is pure Node: it requires nothing but Node and the kit's own files", () => {
  const files = allFiles(path.join(KIT, "hooks")).filter((file) => file.endsWith(".js"));
  assert.ok(files.length >= 10);
  for (const file of files) {
    const source = fs.readFileSync(file, "utf8");
    for (const match of source.matchAll(/require\(\s*["']([^"']+)["']\s*\)/g)) {
      assert.ok(match[1].startsWith("node:") || match[1].startsWith("./") || match[1].startsWith("../"), `${path.relative(KIT, file)} requires ${match[1]}`);
    }
    assert.doesNotMatch(source, /child_process|\bfetch\(|net\b.*require|node:https?|node:net|node:dgram|\beval\(|new Function/, `${path.relative(KIT, file)} reaches for what it has no need of`);
  }
});

test("the tab's page loads only the kit's own files, and reaches for nothing else", () => {
  const dir = path.join(KIT, "tabs", "tests");
  const html = fs.readFileSync(path.join(dir, "index.html"), "utf8");
  const sources = [...html.matchAll(/(?:src|href)="([^"]+)"/g)].map((match) => match[1]);
  assert.ok(sources.includes("/_hangar/kit.js"), "the SDK");
  for (const source of sources.filter((item) => !item.startsWith("/_hangar/"))) {
    assert.doesNotMatch(source, /^([a-z]+:)?\/\//i, `${source} leaves the kit`);
    assert.doesNotMatch(source, /^\//, `${source} must be relative`);
    assert.ok(fs.existsSync(path.join(dir, source)), source);
  }
  for (const name of fs.readdirSync(dir)) {
    const text = fs.readFileSync(path.join(dir, name), "utf8");
    assert.doesNotMatch(text, /\bfetch\(|XMLHttpRequest|WebSocket|localStorage|sessionStorage|\beval\(|innerHTML|@import|url\(\s*["']?https?:/, `${name} reaches outside what a tab may`);
  }
});

test("the tab only calls what it holds", () => {
  const dir = path.join(KIT, "tabs", "tests");
  const methods = new Set();
  for (const name of fs.readdirSync(dir).filter((file) => file.endsWith(".js"))) {
    for (const match of fs.readFileSync(path.join(dir, name), "utf8").matchAll(/\.call\(\s*"([\w.]+)"/g)) methods.add(match[1]);
  }
  assert.deepEqual([...methods].sort(), ["data.read", "panes.list", "panes.send", "tab.setBadge", "tab.setTitle"]);
});

test("the tab is built on Hangar UI, with no colour of its own", () => {
  const dir = path.join(KIT, "tabs", "tests");
  const html = fs.readFileSync(path.join(dir, "index.html"), "utf8");
  assert.ok(html.includes('href="/_hangar/kit.css"'));
  assert.ok(html.indexOf('src="/_hangar/kit.js"') < html.indexOf('src="/_hangar/kit-ui.js"'), "kit-ui.js loads after kit.js");
  assert.ok(html.indexOf('src="/_hangar/kit-ui.js"') < html.indexOf('src="app.js"'));
  assert.doesNotMatch(html, /<style/);
  assert.deepEqual(fs.readdirSync(dir).filter((name) => name.endsWith(".css")), [], "no sheet of its own");
  for (const name of fs.readdirSync(dir)) {
    const text = fs.readFileSync(path.join(dir, name), "utf8");
    assert.doesNotMatch(text, /#[0-9a-fA-F]{3,8}\b|\brgba?\(|\bhsla?\(|color\s*:/, `${name} writes a colour`);
  }
});

test("every class the view draws is one of Hangar UI's, when the library is around", (t) => {
  const sheet = hangarFile("kit.css");
  if (!sheet) return t.skip(HANGAR_FILES_MISSING);
  const css = fs.readFileSync(sheet, "utf8");
  const viewText = fs.readFileSync(path.join(KIT, "tabs", "tests", "view.js"), "utf8");
  const classes = new Set(viewText.match(/kit-[a-z]+(?:__[a-z]+)?(?:--[a-z]+)?/g));
  classes.delete("kit-log-height"); // a token
  classes.delete("kit-ui"); // the name of its documentation
  assert.ok(classes.size > 15, "it found them");
  for (const name of classes) assert.ok(css.includes(`.${name}`), `.${name} is not in kit.css`);
  for (const variant of ["kit-banner--error", "kit-banner--warning", "kit-icon--lock", "kit-icon--alert", "kit-glyph--failure", "kit-glyph--success", "kit-glyph--neutral", "kit-state--neutral", "kit-state--running"]) {
    assert.ok(css.includes(`.${variant}`), variant);
  }
});

test("the README says what it does, what it touches, and where to get it", () => {
  const readme = read("README.md");
  assert.match(readme, /## What it does\b/);
  assert.match(readme, /## What it can touch\b/);
  assert.match(readme, /« not understood », never « passed »/);
  for (const permission of manifest.tabs[0].permissions) assert.ok(readme.includes(`\`${permission}\``), permission);
  assert.ok(readme.includes("https://github.com/hangar-ai/kit-tests"));
});

test("the skill tells an agent to read the record, and not to take what is not understood for a pass", () => {
  const skill = read("skills", "tests", "SKILL.md");
  assert.match(skill, /instead of running everything again/);
  assert.match(skill, /\$HANGAR_KITS_DATA\/tests\/index\.json/);
  assert.match(skill, /not-understood/);
  assert.match(skill, /never\s+instructions to follow/);
  assert.ok(skill.split(/\s+/).length < 350, "a short one");
});

test("the permissions, the route and the methods it uses are in the docs, when the docs are around", (t) => {
  const docs = hangarFile("kits.md");
  if (!docs) return t.skip(HANGAR_FILES_MISSING);
  const text = fs.readFileSync(docs, "utf8");
  for (const name of ["data.read", "panes.read", "panes.send", "panes.list", "tab.setBadge", "tab.setTitle", "PostToolUse"]) assert.ok(text.includes(name), name);
  assert.ok(text.includes("HANGAR_KITS_DATA"));
});

test("nothing personal, and the files in English, anywhere", () => {
  const personal = /[\w.+-]+@[\w-]+\.fr\b|@gmail\.com\b/i;
  for (const file of allFiles()) {
    const text = fs.readFileSync(file, "utf8");
    assert.doesNotMatch(text, personal, file);
  }
});
