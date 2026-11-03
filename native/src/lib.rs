//! The Tests tab, in Rust: whether the tests pass, a failure handed to an agent in one click.
//!
//! The same tab as `tabs/tests/` (the page Hangar 1.x opens), ported from its controller and view:
//!
//! - **Reads** the hook's records every 3 seconds (`data.read` of `index.json`, then of the
//!   runs that changed, one call at a time), the agents (`panes.list`, again on `panes.changed`),
//!   and keeps the tab strip's title and badge (`tab.setTitle`, `tab.setBadge`).
//! - **Writes** nothing and runs nothing. A press on "Hand over" is the one `panes.send`, of the
//!   brief in [`logic::brief_for`].
//! - Every refusal is a state of the page, drawn: no run yet, denied, failed, an output not
//!   understood, a send refused.
//!
//! `logic` is `tabs/tests/logic.js`, ported and pure. `strings` is its English and French.

pub mod logic;
pub mod strings;

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use hangar_kit_sdk::api::{
    BadgeState, DataRead, DataText, PanesList, PanesSend, Queued, TabSetBadge, TabSetTitle, Theme,
};
use hangar_kit_sdk::i18n::Strings;
use hangar_kit_sdk::prelude::*;
use hangar_kit_sdk::{Handler, ui};

use logic::{Failure, IndexEntry, Overall, Run, Status as RunStatus};

/// How often the hook's records are read again.
pub const POLL: Duration = Duration::from_secs(3);

/// How many of a run's failures are drawn before "Show N more". A run keeps 50 at most, of 2 000
/// characters: forty runs of them would be about 4 MB, the most a view may be.
pub const FOLDED: usize = 5;

/// The longest text of the tab strip's badge.
const BADGE_MAX: usize = 12;

type Item = ui::ListItem<Handler<Message>>;

/// What the page is, before its list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Loading,
    List,
    Denied,
    Error,
}

/// Where a poll is. One at a time: a poll is a few calls in a row, never more than one waiting,
/// where Hangar holds 16.
#[derive(Debug)]
enum Poll {
    Idle,
    /// `index.json` asked for.
    Index,
    /// The runs of the index, read one after the other.
    Runs(Load),
}

#[derive(Debug)]
struct Load {
    index: Vec<IndexEntry>,
    next: usize,
    runs: Vec<Run>,
}

/// A hand-over that went through: to whom, and which run it was about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sent {
    pub agent: String,
    pub run_at: i64,
}

/// A hand-over that did not: `problem.<code>` says why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub code: &'static str,
    pub detail: String,
    pub agent: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Badge {
    state: BadgeState,
    text: String,
}

pub struct Tests {
    t: Strings,
    utc_offset: i32,
    now: i64,
    screen: Screen,
    runs: Vec<Run>,
    panes: Vec<api::Pane>,
    panes_denied: bool,
    /// Run key → the agent the person picked.
    choice: BTreeMap<String, String>,
    /// Ids (`<key>:<index>` or `<key>:all`) being sent.
    pending: BTreeSet<String>,
    sent: BTreeMap<String, Sent>,
    problems: BTreeMap<String, Problem>,
    /// The runs whose failures are all drawn, past the first few.
    expanded: BTreeSet<String>,
    /// The last read failed, and what is listed is from before.
    stale: bool,
    poll: Poll,
    last_index: Option<Vec<IndexEntry>>,
    last_badge: Option<String>,
}

#[derive(Clone, Debug)]
pub enum Message {
    /// The 3 s timer: read the index again.
    Poll,
    Index(Result<DataText, KitError>),
    /// The file of the run of this key.
    Run(String, Result<DataText, KitError>),
    LoadPanes,
    Panes(Result<Vec<api::Pane>, KitError>),
    /// A run, and the agent picked for it.
    Choose(String, String),
    /// A run, and one of its failures (`None`: its first few).
    HandOver(String, Option<usize>),
    Sent {
        id: String,
        agent: String,
        run_at: i64,
        answer: Result<Queued, KitError>,
    },
    Retry,
    ShowMore(String),
    ShowFewer(String),
    CopyCommand(String),
    CopyMessage(String, usize),
    Theme(Theme),
}

/// What a hand-over is called, in `pending`, `sent` and `problems`.
fn hand_id(key: &str, index: Option<usize>) -> String {
    match index {
        Some(index) => format!("{key}:{index}"),
        None => format!("{key}:all"),
    }
}

/// The run an id is about.
fn key_of(id: &str) -> &str {
    id.split(':').next().unwrap_or("")
}

/// What the tab holds, for the tests to read: the page is what a person sees, this is what it
/// is made of.
impl Tests {
    pub fn screen(&self) -> Screen {
        self.screen
    }

    /// The runs listed, as the index orders them.
    pub fn runs(&self) -> &[Run] {
        &self.runs
    }

    pub fn panes(&self) -> &[api::Pane] {
        &self.panes
    }

    /// `panes.list` was refused: the tab may not list the agents.
    pub fn panes_denied(&self) -> bool {
        self.panes_denied
    }

    /// The hand-overs that went through, by id (`<key>:<index>` or `<key>:all`).
    pub fn sent(&self) -> &BTreeMap<String, Sent> {
        &self.sent
    }

    /// The hand-overs that were refused, by id.
    pub fn problems(&self) -> &BTreeMap<String, Problem> {
        &self.problems
    }

    /// The hand-overs on their way.
    pub fn pending(&self) -> &BTreeSet<String> {
        &self.pending
    }
}

impl Tests {
    fn language(&self) -> &'static str {
        logic::pick_lang(self.t.locale())
    }

    /// The agent a run goes to: the one the person picked, else the one that ran it.
    fn target_of(&self, run: &Run) -> Option<String> {
        self.choice
            .get(&run.key)
            .filter(|agent| !agent.is_empty())
            .cloned()
            .or_else(|| logic::default_target(run, &self.panes))
    }

    /// The tab strip's badge for the runs.
    fn badge(&self) -> Option<Badge> {
        match logic::overall(&self.runs) {
            Overall::None => None,
            Overall::Failed(n) => Some(Badge {
                state: BadgeState::Failure,
                text: if n > 0 {
                    self.t
                        .tn("badgeFailed", n as i64, &[])
                        .chars()
                        .take(BADGE_MAX)
                        .collect()
                } else {
                    "!".to_owned()
                },
            }),
            Overall::Green => Some(Badge {
                state: BadgeState::Success,
                text: self.t.t("badgeOk"),
            }),
            Overall::Unknown => Some(Badge {
                state: BadgeState::Neutral,
                text: "?".to_owned(),
            }),
        }
    }

    /// Sets the badge, unless it already is that.
    fn set_badge(&mut self, badge: Option<Badge>, cx: &mut Context<Message>) {
        let id = badge.as_ref().map_or_else(
            || "none".to_owned(),
            |badge| format!("{:?}|{}", badge.state, badge.text),
        );
        if self.last_badge.as_deref() == Some(id.as_str()) {
            return;
        }
        self.last_badge = Some(id);
        cx.call_quietly(TabSetBadge {
            state: badge.as_ref().map(|badge| badge.state),
            text: badge.map(|badge| badge.text),
        });
    }

    fn start_poll(&mut self, cx: &mut Context<Message>) {
        if matches!(self.poll, Poll::Idle) {
            self.poll = Poll::Index;
            cx.call(
                DataRead {
                    path: logic::INDEX.into(),
                },
                Message::Index,
            );
        }
    }

    /// A read went wrong. `denied` is a state of its own, with no badge; a failure after a good
    /// read leaves the list where it is, marked; before one, it is the page.
    fn poll_failed(&mut self, error: &KitError, cx: &mut Context<Message>) {
        self.poll = Poll::Idle;
        if error.is(KitError::DENIED) {
            self.screen = Screen::Denied;
            self.last_index = None;
            self.runs.clear();
            self.set_badge(None, cx);
        } else if self.screen == Screen::List {
            self.stale = true;
        } else {
            self.screen = Screen::Error;
        }
    }

    /// Asks for the next run of the index that is not known yet; the known ones are those of the
    /// list as it is, same time.
    fn read_next(&mut self, mut load: Load, cx: &mut Context<Message>) {
        while let Some(entry) = load.index.get(load.next) {
            if let Some(known) = self
                .runs
                .iter()
                .find(|run| run.key == entry.key && run.at == entry.at)
            {
                load.runs.push(known.clone());
                load.next += 1;
                continue;
            }
            let key = entry.key.clone();
            cx.call(
                DataRead {
                    path: logic::run_path(&key),
                },
                move |answer| Message::Run(key, answer),
            );
            self.poll = Poll::Runs(load);
            return;
        }
        self.finish(load, cx);
    }

    /// Every run of the index is read: it is the list now.
    fn finish(&mut self, load: Load, cx: &mut Context<Message>) {
        self.poll = Poll::Idle;
        self.last_index = Some(load.index);
        self.runs = load.runs;
        // What was handed over, or went wrong, may be about a run that has since been replaced.
        let runs = &self.runs;
        let run_of = |id: &str| runs.iter().find(|run| run.key == key_of(id));
        self.sent
            .retain(|id, sent| run_of(id).is_some_and(|run| run.at == sent.run_at));
        self.problems.retain(|id, _| run_of(id).is_some());
        self.expanded
            .retain(|key| runs.iter().any(|run| run.key == *key));
        let badge = self.badge();
        self.set_badge(badge, cx);
        self.screen = Screen::List;
        self.stale = false;
    }

    fn hand_over(&mut self, key: &str, index: Option<usize>, cx: &mut Context<Message>) {
        let Some(run) = self.runs.iter().find(|run| run.key == key) else {
            return;
        };
        let id = hand_id(key, index);
        let Some(agent) = self.target_of(run) else {
            return;
        };
        if self.pending.contains(&id) {
            return;
        }
        let failure = match index {
            Some(index) => match run.failures.get(index) {
                Some(failure) => Some(failure),
                None => return,
            },
            None => None,
        };
        let text = logic::brief_for(run, failure);
        let run_at = run.at;
        self.pending.insert(id.clone());
        self.problems.remove(&id);
        cx.call(
            PanesSend {
                pane: agent.clone(),
                text,
            },
            move |answer| Message::Sent {
                id,
                agent,
                run_at,
                answer,
            },
        );
    }
}

impl Kit for Tests {
    type Message = Message;

    fn init(start: &Start, cx: &mut Context<Message>) -> Self {
        let t = strings::strings(&start.theme.locale);
        cx.call_quietly(TabSetTitle { title: t.t("tab") });
        cx.call(
            DataRead {
                path: logic::INDEX.into(),
            },
            Message::Index,
        );
        cx.call(PanesList {}, Message::Panes);
        Tests {
            t,
            utc_offset: start.utc_offset,
            now: cx.now(),
            screen: Screen::Loading,
            runs: Vec::new(),
            panes: Vec::new(),
            panes_denied: false,
            choice: BTreeMap::new(),
            pending: BTreeSet::new(),
            sent: BTreeMap::new(),
            problems: BTreeMap::new(),
            expanded: BTreeSet::new(),
            stale: false,
            poll: Poll::Index,
            last_index: None,
            last_badge: None,
        }
    }

    fn update(&mut self, message: Message, cx: &mut Context<Message>) {
        self.now = cx.now();
        match message {
            Message::Poll => self.start_poll(cx),
            Message::Retry => {
                self.start_poll(cx);
                cx.call(PanesList {}, Message::Panes);
            }
            Message::Index(answer) => {
                // No index yet is no run yet, not an error.
                let source = match answer {
                    Ok(read) => read.text,
                    Err(error) if error.is(KitError::NOT_FOUND) => String::new(),
                    Err(error) => return self.poll_failed(&error, cx),
                };
                let index = logic::parse_index(&source);
                if self.last_index.as_ref() == Some(&index) {
                    self.poll = Poll::Idle;
                    self.screen = Screen::List;
                    self.stale = false;
                } else {
                    self.read_next(
                        Load {
                            index,
                            next: 0,
                            runs: Vec::new(),
                        },
                        cx,
                    );
                }
            }
            Message::Run(_, answer) => {
                let Poll::Runs(mut load) = std::mem::replace(&mut self.poll, Poll::Idle) else {
                    return;
                };
                match answer {
                    // A file that is not a run is skipped, not fatal; so is one that is gone.
                    Ok(read) => load.runs.extend(logic::parse_run(&read.text)),
                    Err(error) if error.is(KitError::NOT_FOUND) => {}
                    Err(error) => return self.poll_failed(&error, cx),
                }
                load.next += 1;
                self.read_next(load, cx);
            }
            Message::LoadPanes => cx.call(PanesList {}, Message::Panes),
            Message::Panes(answer) => match answer {
                Ok(panes) => {
                    self.panes = panes;
                    self.panes_denied = false;
                }
                Err(error) => {
                    self.panes = Vec::new();
                    self.panes_denied = error.is(KitError::DENIED);
                }
            },
            Message::Choose(key, agent) => {
                self.choice.insert(key, agent);
            }
            Message::HandOver(key, index) => self.hand_over(&key, index, cx),
            Message::Sent {
                id,
                agent,
                run_at,
                answer,
            } => {
                self.pending.remove(&id);
                match answer {
                    Ok(_) => {
                        self.sent.insert(id, Sent { agent, run_at });
                    }
                    Err(error) => {
                        let code = match error.code.as_str() {
                            KitError::DENIED => KitError::DENIED,
                            KitError::NOT_FOUND => KitError::NOT_FOUND,
                            KitError::RATE_LIMITED => KitError::RATE_LIMITED,
                            KitError::UNAVAILABLE => KitError::UNAVAILABLE,
                            _ => KitError::FAILED,
                        };
                        self.problems.insert(
                            id,
                            Problem {
                                code,
                                detail: error.message,
                                agent,
                            },
                        );
                        // The agent is gone, or busy: the list of agents is out of date.
                        if code == KitError::NOT_FOUND || code == KitError::UNAVAILABLE {
                            cx.call(PanesList {}, Message::Panes);
                        }
                    }
                }
            }
            Message::ShowMore(key) => {
                self.expanded.insert(key);
            }
            Message::ShowFewer(key) => {
                self.expanded.remove(&key);
            }
            Message::CopyCommand(key) => {
                if let Some(run) = self.runs.iter().find(|run| run.key == key) {
                    cx.copy(run.command.clone());
                    cx.toast(Notice::Success, self.t.t("copied"));
                }
            }
            Message::CopyMessage(key, index) => {
                let found = self.runs.iter().find(|run| run.key == key);
                if let Some(failure) = found.and_then(|run| run.failures.get(index)) {
                    cx.copy(failure.message.clone());
                    cx.toast(Notice::Success, self.t.t("copied"));
                }
            }
            Message::Theme(theme) => {
                let before = self.language();
                self.t.set_locale(theme.locale);
                // The strip's title and badge follow Hangar's language.
                if self.language() != before {
                    cx.call_quietly(TabSetTitle {
                        title: self.t.t("tab"),
                    });
                    self.last_badge = None;
                    let badge = self.badge();
                    self.set_badge(badge, cx);
                }
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let t = &self.t;
        let mut head = header::<Message>(t.t("tab"));
        if self.screen == Screen::List {
            head = head.state(self.overall_state());
        }
        let mut page_children: Vec<Element<Message>> = vec![head.into()];
        match self.screen {
            Screen::Loading => page_children.push(loading(t.t("loading")).into()),
            Screen::Denied => page_children.push(
                denied(t.t("deniedTitle"))
                    .text(t.t("deniedText"))
                    .action(button(t.t("retry")).small().on_press(Message::Retry))
                    .into(),
            ),
            Screen::Error => page_children.push(
                failed(t.t("errorTitle"))
                    .text(t.t("errorText"))
                    .action(
                        button(t.t("retry"))
                            .small()
                            .primary()
                            .on_press(Message::Retry),
                    )
                    .into(),
            ),
            Screen::List => page_children.extend(self.list()),
        }
        page(page_children).into()
    }

    fn subscriptions(&self) -> Subscriptions<Message> {
        Subscriptions::none()
            .every("poll", POLL, Message::Poll)
            .panes_changed(Message::LoadPanes)
            .theme(Message::Theme)
    }
}

// ---------------------------------------------------------------- The page.

impl Tests {
    /// The header's state: what the person wants to know first.
    fn overall_state(&self) -> Element<Message> {
        let t = &self.t;
        match logic::overall(&self.runs) {
            Overall::Failed(n) if n > 0 => {
                state_label(Status::Failure, t.tn("stateFailed", n as i64, &[])).into()
            }
            Overall::Failed(_) => state_label(Status::Failure, t.t("failedLabel")).into(),
            Overall::Green => state_label(Status::Success, t.t("stateGreen")).into(),
            Overall::Unknown => state_label(Status::Neutral, t.t("stateUnknown")).into(),
            Overall::None => state_label(Status::Neutral, t.t("stateNone")).into(),
        }
    }

    fn list(&self) -> Vec<Element<Message>> {
        let t = &self.t;
        let mut out: Vec<Element<Message>> = Vec::new();
        if self.stale {
            out.push(banner(Notice::Warning, t.t("staleWarning")).into());
        }
        if self.runs.is_empty() {
            out.push(empty(t.t("emptyTitle")).text(t.t("emptyText")).into());
            return out;
        }
        for group in logic::group_runs(&self.runs) {
            let blocks: Vec<Element<Message>> =
                group.runs.iter().map(|run| self.run_block(run)).collect();
            let mut section = section(group.project, blocks);
            if group.failed > 0 {
                section = section.count(group.failed.to_string());
            }
            out.push(section.into());
        }
        out
    }

    /// A run: its summary and its failures as one group of rows, then who they go to.
    fn run_block(&self, run: &Run) -> Element<Message> {
        let t = &self.t;
        let total = run.failures.len();
        let shown = if self.expanded.contains(&run.key) {
            total
        } else {
            total.min(FOLDED)
        };
        let mut items = vec![self.summary(run)];
        items.extend(
            run.failures
                .iter()
                .take(shown)
                .enumerate()
                .map(|(index, failure)| self.failure(run, index, failure)),
        );
        let mut block: Vec<Element<Message>> = vec![list(items).group().into()];
        if total > 0 {
            let mut foot: Vec<Element<Message>> = Vec::new();
            if shown < total {
                let hidden = (total - shown).to_string();
                foot.push(
                    row![
                        button(t.tv("showMore", &[("n", &hidden)]))
                            .small()
                            .ghost()
                            .on_press(Message::ShowMore(run.key.clone()))
                    ]
                    .into(),
                );
            } else if total > FOLDED {
                foot.push(
                    row![
                        button(t.t("showFewer"))
                            .small()
                            .ghost()
                            .on_press(Message::ShowFewer(run.key.clone()))
                    ]
                    .into(),
                );
            }
            foot.push(self.picker(run));
            if total > 1 {
                if let Some(problem) = self.problem(&hand_id(&run.key, None)) {
                    foot.push(problem);
                }
                let label = t.tn("handOverAll", total as i64, &[]);
                foot.push(row![self.hand_over_control(run, None, label)].into());
            }
            block.push(column(foot).spacing(6.0).into());
        }
        column(block).spacing(8.0).into()
    }

    /// `3 failed · 12 passed · 1 skipped · 2.3 s`: only what the run said.
    fn counts(&self, run: &Run, failed: Option<u64>) -> String {
        let t = &self.t;
        let mut parts: Vec<String> = Vec::new();
        if let Some(n) = failed.filter(|n| *n > 0) {
            parts.push(t.tn("failedCount", n as i64, &[]));
        }
        if let Some(n) = run.passed {
            parts.push(t.tn("passedCount", n as i64, &[]));
        }
        if let Some(n) = run.skipped.filter(|n| *n > 0) {
            parts.push(t.tn("skippedCount", n as i64, &[]));
        }
        if let Some(ms) = run.duration {
            parts.push(format::duration(ms, t.locale()));
        }
        parts.join(" · ")
    }

    /// The row that says what a run was.
    fn summary(&self, run: &Run) -> Item {
        let t = &self.t;
        let (status, label) = match run.status {
            RunStatus::Passed => (Status::Success, t.t("passedLabel")),
            RunStatus::Failed => (Status::Failure, t.t("failedLabel")),
            RunStatus::NotUnderstood => (Status::Neutral, t.t("unknownLabel")),
        };
        let title = match &run.framework {
            Some(framework) => t.tv(
                "runLabel",
                &[("agent", &run.agent), ("framework", framework)],
            ),
            None => t.tv("runLabelNone", &[("agent", &run.agent)]),
        };
        let line = match run.status {
            RunStatus::Passed if run.failed == Some(0) => {
                [t.t("allGreen"), self.counts(run, Some(0))]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join(" · ")
            }
            RunStatus::NotUnderstood => String::new(),
            _ => self.counts(run, run.failed),
        };
        let when = format::since(run.at, self.now, self.utc_offset, t.locale());
        let facts = if line.is_empty() {
            when
        } else {
            format!("{line} · {when}")
        };

        let mut entry = item(title)
            .wrap()
            .lead(glyph(status, label))
            .sub(facts)
            .tooltip(format::date(run.at, self.now, self.utc_offset, t.locale()))
            .action(
                icon_button(Icon::Copy, t.t("copyCommand"))
                    .small()
                    .on_press(Message::CopyCommand(run.key.clone())),
            )
            .body(text(run.command.clone()).code());

        let failed = run.failed.unwrap_or(0);
        if run.status == RunStatus::NotUnderstood {
            let words = if run.reason.as_deref() == Some("no-tests") {
                "noTests"
            } else {
                "notUnderstood"
            };
            entry = entry.body(banner(Notice::Info, t.t(words)));
        }
        if run.partial {
            entry = entry.body(banner(Notice::Warning, t.t("partial")));
        }
        if run.status == RunStatus::Failed {
            if run.failed == Some(0)
                && run.failures.is_empty()
                && !run.partial
                && let Some(code) = run.exit_code
            {
                let code = code.to_string();
                entry = entry.body(banner(Notice::Info, t.tv("exitCode", &[("code", &code)])));
            }
            if run.failures.is_empty() && failed > 0 {
                entry = entry.body(banner(Notice::Info, t.t("noDetail")));
            }
            if !run.failures.is_empty() && failed > run.failures.len() as u64 {
                let (n, total) = (run.failures.len().to_string(), failed.to_string());
                entry = entry.body(
                    text(t.tv("showing", &[("n", &n), ("total", &total)]))
                        .small()
                        .dim(),
                );
            }
        }
        entry
    }

    /// The row of one failure: its place, its message, and its own button.
    fn failure(&self, run: &Run, index: usize, failure: &Failure) -> Item {
        let t = &self.t;
        let mut entry = item(failure.name.clone())
            .wrap()
            .lead(glyph(Status::Failure, t.t("failedLabel")));
        let place = logic::where_of(failure);
        if !place.is_empty() {
            entry = entry.sub(place);
        }
        if !failure.message.is_empty() {
            entry = entry
                .body(pre(failure.message.clone()).height(160.0))
                .action(
                    icon_button(Icon::Copy, t.t("copyMessage"))
                        .small()
                        .on_press(Message::CopyMessage(run.key.clone(), index)),
                );
        }
        if let Some(problem) = self.problem(&hand_id(&run.key, Some(index))) {
            entry = entry.body(problem);
        }
        entry.body(row![self.hand_over_control(
            run,
            Some(index),
            t.t("handOver")
        )])
    }

    /// Why a hand-over did not go, if it did not.
    fn problem(&self, id: &str) -> Option<Element<Message>> {
        let problem = self.problems.get(id)?;
        let key = format!("problem.{}", problem.code);
        let words = self.t.tv(
            &key,
            &[("agent", &problem.agent), ("detail", &problem.detail)],
        );
        Some(banner(Notice::Error, words).into())
    }

    /// The button that hands failure `index` (or the first few of a run, when none) to the run's
    /// agent, or what stands in for it.
    fn hand_over_control(
        &self,
        run: &Run,
        index: Option<usize>,
        label: String,
    ) -> Element<Message> {
        let t = &self.t;
        let id = hand_id(&run.key, index);
        if let Some(sent) = self.sent.get(&id) {
            return state_label(Status::Success, t.tv("sentTo", &[("agent", &sent.agent)])).into();
        }
        let pending = self.pending.contains(&id);
        let ready = !pending && self.target_of(run).is_some();
        button(if pending { t.t("handingOver") } else { label })
            .small()
            .on_press_maybe(ready.then(|| Message::HandOver(run.key.clone(), index)))
            .into()
    }

    /// One line saying who the failures go to: a choice when there are several agents.
    fn picker(&self, run: &Run) -> Element<Message> {
        let t = &self.t;
        if self.panes_denied {
            return text(t.t("panesDenied")).small().dim().into();
        }
        let agents = logic::candidates(&self.panes);
        match agents.as_slice() {
            [] => text(t.t("noAgent")).small().dim().into(),
            [only] => text(t.tv("agentIs", &[("name", &only.name)]))
                .small()
                .dim()
                .into(),
            _ => {
                let key = run.key.clone();
                select(
                    agents
                        .iter()
                        .map(|pane| select_option(pane.name.clone(), pane.name.clone())),
                    self.target_of(run),
                )
                .label(t.t("agentLabel"))
                .width(240.0)
                .on_select(move |agent| Message::Choose(key.clone(), agent))
                .into()
            }
        }
    }
}

export_kit!(Tests);
