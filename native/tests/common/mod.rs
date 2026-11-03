//! Shared by the tests: the hook's records as the tab reads them, and a fake Hangar answering
//! the tab's calls over an in-memory data folder (`fakeHangar` and `fakeClock` of
//! `test/helpers.js`).
#![allow(dead_code)]

use std::collections::BTreeMap;

use serde_json::{Value, json};

use hangar_kit_sdk::api::KitError;
use hangar_kit_sdk::format;
use hangar_kit_sdk::testing::{self, Harness, StartExt};
use hangar_kit_sdk::ui::abi::Call;
use hangar_kit_sdk::ui::{self, Element as Node, HandlerId};

use kit_tests_tab::Tests;
use kit_tests_tab::logic::{self, Run};

pub const KEY_A: &str = "aaaaaaaaaaaaaaaa";
pub const KEY_B: &str = "bbbbbbbbbbbbbbbb";
pub const KEY_C: &str = "cccccccccccccccc";

/// `2026-10-31T17:02:00Z`, the moment of every test.
pub fn now() -> i64 {
    format::parse_iso("2026-10-31T17:02:00Z").expect("a date")
}

pub fn failure(name: &str, message: &str, file: &str, line: i64) -> Value {
    json!({ "name": name, "message": message, "file": file, "line": line })
}

/// A failed run of vitest by Emma in `app`, two failures, a minute ago: `overrides` replace
/// fields of it (a `null` is a field the hook did not know).
pub fn run(overrides: Value) -> Value {
    let mut run = json!({
        "version": 1,
        "key": KEY_A,
        "at": now() - 60_000,
        "status": "failed",
        "framework": "vitest",
        "passed": 10,
        "failed": 2,
        "skipped": 1,
        "duration": 2300,
        "failures": [
            failure("math > adds", "math > adds failed", "src/a.test.js", 4),
            failure("math > throws", "Error: kaboom", "src/b.test.js", 9),
        ],
        "partial": false,
        "exitCode": 1,
        "command": "pnpm vitest run",
        "cwd": "/work/app",
        "project": "app",
        "agent": "Emma",
    });
    for (name, value) in overrides.as_object().expect("an object") {
        run[name] = value.clone();
    }
    run
}

/// A run passed: nothing failed, nothing to hand over.
pub fn passed(overrides: Value) -> Value {
    let mut green = run(json!({ "status": "passed", "failed": 0, "failures": [] }));
    for (name, value) in overrides.as_object().expect("an object") {
        green[name] = value.clone();
    }
    green
}

/// The run as the tab reads it.
pub fn parsed(run: &Value) -> Run {
    logic::parse_run(&run.to_string()).expect("a run")
}

pub fn pane(name: &str) -> Value {
    json!({ "name": name, "agent": "claude", "role": "worker", "state": "idle", "canSend": true })
}

pub fn pane_with(name: &str, extra: Value) -> Value {
    let mut pane = pane(name);
    for (field, value) in extra.as_object().expect("an object") {
        pane[field] = value.clone();
    }
    pane
}

fn index(runs: &[Value]) -> String {
    let entries: Vec<Value> = runs
        .iter()
        .map(|run| {
            json!({ "key": run["key"], "at": run["at"], "status": run["status"],
                    "project": run["project"], "agent": run["agent"] })
        })
        .collect();
    json!({ "version": 1, "runs": entries }).to_string()
}

/// A data folder as the hook leaves it for these runs.
pub fn files(runs: &[Value]) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    files.insert("index.json".to_owned(), index(runs));
    for run in runs {
        let key = run["key"].as_str().expect("a key");
        files.insert(format!("runs/{key}.json"), run.to_string());
    }
    files
}

/// Hangar, as far as this tab needs it. `pump` answers what the tab asks, and keeps a log.
pub struct Fake {
    pub files: BTreeMap<String, String>,
    pub panes: Vec<Value>,
    pub deny_read: bool,
    pub deny_panes: bool,
    pub read_error: Option<String>,
    pub send_error: Option<String>,
    /// Every call answered, in order.
    pub log: Vec<Call>,
}

impl Fake {
    pub fn new(runs: &[Value]) -> Fake {
        Fake {
            files: files(runs),
            panes: vec![pane("Emma"), pane("Jack")],
            deny_read: false,
            deny_panes: false,
            read_error: None,
            send_error: None,
            log: Vec::new(),
        }
    }

    /// The data folder now holds these runs.
    pub fn set_runs(&mut self, runs: &[Value]) {
        self.files = files(runs);
    }

    pub fn calls_to(&self, method: &str) -> Vec<&Call> {
        self.log
            .iter()
            .filter(|call| call.method == method)
            .collect()
    }

    fn answer(&self, call: &Call) -> Result<Value, KitError> {
        let refused = |code: &str, message: &str| Err(KitError::new(code, message));
        match call.method.as_str() {
            "tab.setTitle" | "tab.setBadge" | "ui.copy" => Ok(json!({})),
            "ui.toast" => Ok(json!({ "action": false })),
            "data.read" => {
                if self.deny_read {
                    return refused("denied", "this tab was not granted data.read");
                }
                if let Some(code) = &self.read_error {
                    return refused(code, code);
                }
                let path = call.params["path"].as_str().unwrap_or_default();
                match self.files.get(path) {
                    Some(text) => Ok(json!({ "text": text })),
                    None => refused("not_found", "no such file"),
                }
            }
            "panes.list" => {
                if self.deny_panes {
                    return refused("denied", "this tab was not granted panes.read");
                }
                Ok(Value::Array(self.panes.clone()))
            }
            "panes.send" => match &self.send_error {
                Some(code) => refused(code, &format!("could not: {code}")),
                None => Ok(json!({ "queued": true, "pane": call.params["pane"] })),
            },
            other => refused(
                "denied",
                &format!("this tab was not granted what {other} needs"),
            ),
        }
    }

    /// Answers every call the tab waits on, and those its answers cause, until it waits on none.
    pub fn pump(&mut self, tab: &mut Harness<Tests>) {
        while let Some(call) = tab.calls().first().cloned() {
            let answer = self.answer(&call);
            self.log.push(call.clone());
            tab.reply(call.id, answer);
        }
    }

    /// The 3 s timer, and what it asks.
    pub fn poll(&mut self, tab: &mut Harness<Tests>) {
        tab.timer("poll");
        self.pump(tab);
    }
}

/// The tab, started as Hangar does it with the three permissions it asks for, and everything
/// it asked for answered.
pub fn started(fake: &mut Fake) -> Harness<Tests> {
    started_as(fake, testing::start())
}

pub fn started_as(fake: &mut Fake, start: ui::abi::Start) -> Harness<Tests> {
    testing::set_now(Some(now()));
    let mut tab = Harness::<Tests>::new(start.grant(["data.read", "panes.read", "panes.send"]));
    fake.pump(&mut tab);
    tab
}

/// What the page holds, whatever the widget.
pub fn collect<T>(tab: &Harness<Tests>, mut pick: impl FnMut(&Node) -> Option<T>) -> Vec<T> {
    let mut found = Vec::new();
    tab.view().root.for_each(&mut |element| {
        if let Some(item) = pick(element) {
            found.push(item);
        }
    });
    found
}

/// The buttons named `label` (or whose tooltip is), in the order of the page.
pub fn buttons(tab: &Harness<Tests>, label: &str) -> Vec<ui::Button> {
    collect(tab, |element| match element {
        Node::Button(button)
            if button.label.as_deref() == Some(label)
                || button.tooltip.as_deref() == Some(label) =>
        {
            Some(button.clone())
        }
        _ => None,
    })
}

/// The handler of the `n`th button named `label`, as the page gives it now.
pub fn handler_of(tab: &Harness<Tests>, label: &str, n: usize) -> HandlerId {
    buttons(tab, label)[n]
        .on_press
        .unwrap_or_else(|| panic!("button {n} {label:?} is disabled"))
}

/// Presses the `n`th button named `label`.
pub fn press_nth(tab: &mut Harness<Tests>, label: &str, n: usize) {
    let handler = handler_of(tab, label, n);
    tab.ui(handler, None);
}

pub fn states(tab: &Harness<Tests>) -> Vec<ui::Status> {
    collect(tab, |element| match element {
        Node::State(state) => Some(state.status),
        _ => None,
    })
}

pub fn glyphs(tab: &Harness<Tests>) -> Vec<ui::Status> {
    collect(tab, |element| match element {
        Node::Glyph(glyph) => Some(glyph.status),
        _ => None,
    })
}

pub fn banners(tab: &Harness<Tests>, kind: ui::Notice) -> usize {
    collect(tab, |element| match element {
        Node::Banner(banner) if banner.kind == kind => Some(()),
        _ => None,
    })
    .len()
}

pub fn section_titles(tab: &Harness<Tests>) -> Vec<String> {
    collect(tab, |element| match element {
        Node::Section(section) => Some(section.title.clone()),
        _ => None,
    })
}

pub fn select(tab: &Harness<Tests>) -> Option<ui::Select> {
    collect(tab, |element| match element {
        Node::Select(select) => Some(select.clone()),
        _ => None,
    })
    .into_iter()
    .next()
}

/// The last `tab.setBadge` the tab made, as its params.
pub fn last_badge(fake: &Fake) -> Value {
    fake.calls_to("tab.setBadge")
        .last()
        .expect("a badge was set")
        .params
        .clone()
}
