//! The tab: `controller.js` and `view.js`, over the harness in place of a fake `window.hangar`
//! and a fake DOM. Every case keeps the sentence of the JavaScript test it comes from.

mod common;

use serde_json::{Value, json};

use common::*;
use hangar_kit_sdk::api::KitError;
use hangar_kit_sdk::testing::{self, Harness, StartExt};
use hangar_kit_sdk::ui::{self, Element as Node};
use kit_tests_tab::logic;
use kit_tests_tab::{Screen, Sent, Tests};

const HAND_OVER: &str = "Hand over to an agent";

fn brief(run: &Value, index: Option<usize>) -> String {
    let run = parsed(run);
    logic::brief_for(&run, index.map(|index| &run.failures[index]))
}

fn sends(fake: &Fake) -> Vec<(String, String)> {
    fake.calls_to("panes.send")
        .iter()
        .map(|call| {
            (
                call.params["pane"].as_str().unwrap().to_owned(),
                call.params["text"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

fn root(tab: &Harness<Tests>) -> Value {
    serde_json::to_value(&tab.view().root).unwrap()
}

// ---- the strip: title and badge ----

/// it reads the index, then each run, and sets the title and the badge
#[test]
fn it_reads_the_index_then_each_run_and_sets_the_title_and_the_badge() {
    let mut fake = Fake::new(&[
        run(json!({})),
        passed(json!({ "key": KEY_B, "project": "other" })),
    ]);
    let tab = started(&mut fake);
    assert_eq!(tab.kit().screen(), Screen::List);
    let keys: Vec<&str> = tab
        .kit()
        .runs()
        .iter()
        .map(|run| run.key.as_str())
        .collect();
    assert_eq!(keys, vec![KEY_A, KEY_B]);
    assert_eq!(
        fake.calls_to("tab.setTitle")[0].params,
        json!({ "title": "Tests" })
    );
    assert_eq!(
        last_badge(&fake),
        json!({ "state": "failure", "text": "2 failed" })
    );
    assert_eq!(fake.calls_to("data.read").len(), 3);
    let paths: Vec<&str> = fake
        .calls_to("data.read")
        .iter()
        .map(|call| call.params["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        paths,
        vec![
            "index.json",
            &format!("runs/{KEY_A}.json"),
            &format!("runs/{KEY_B}.json")
        ]
    );
    assert_eq!(
        tab.methods(),
        Vec::<&str>::new(),
        "and nothing is left waiting"
    );
}

/// the badge counts the failures, in the language, within 12 characters
#[test]
fn the_badge_counts_the_failures_in_the_language_within_12_characters() {
    let badge = |runs: Vec<Value>, locale: &str| {
        let mut fake = Fake::new(&runs);
        started_as(&mut fake, testing::start().locale(locale));
        last_badge(&fake)
    };
    assert_eq!(
        badge(vec![run(json!({}))], "en"),
        json!({ "state": "failure", "text": "2 failed" })
    );
    assert_eq!(
        badge(vec![run(json!({}))], "fr"),
        json!({ "state": "failure", "text": "2 échecs" })
    );
    assert_eq!(
        badge(vec![run(json!({ "failed": 12345, "failures": [] }))], "fr"),
        json!({ "state": "failure", "text": "12345 échecs" })
    );
    assert_eq!(
        badge(vec![passed(json!({}))], "en"),
        json!({ "state": "success", "text": "OK" })
    );
    assert_eq!(
        badge(
            vec![run(json!({ "status": "not-understood", "failures": [] }))],
            "en"
        )["state"],
        "neutral"
    );
    assert_eq!(badge(vec![], "en"), json!({ "state": null }));
    let long = badge(
        vec![run(json!({ "failed": 999_999_999, "failures": [] }))],
        "fr",
    );
    assert!(
        long["text"].as_str().unwrap().chars().count() <= 12,
        "{long}"
    );
    let unknown = badge(vec![run(json!({ "failed": null, "failures": [] }))], "en");
    assert_eq!(
        unknown,
        json!({ "state": "failure", "text": "!" }),
        "failed, and nothing counted"
    );
}

/// the language changes the strip's title and badge
#[test]
fn the_language_changes_the_strips_title_and_badge() {
    let mut fake = Fake::new(&[run(json!({}))]);
    let mut tab = started(&mut fake);
    assert!(tab.shows(HAND_OVER));
    tab.event("theme", json!({ "scheme": "dark", "locale": "fr" }));
    fake.pump(&mut tab);
    assert_eq!(
        last_badge(&fake),
        json!({ "state": "failure", "text": "2 échecs" })
    );
    assert_eq!(fake.calls_to("tab.setTitle").len(), 2);
    assert!(tab.shows("Confier à un agent"));
    // The same language again, and a scheme that changed: nothing to say.
    tab.event("theme", json!({ "scheme": "light", "locale": "fr-FR" }));
    fake.pump(&mut tab);
    assert_eq!(fake.calls_to("tab.setTitle").len(), 2);
    assert_eq!(fake.calls_to("tab.setBadge").len(), 2);
}

// ---- the poll ----

/// a poll with nothing new reads only the index; a replaced run is read again
#[test]
fn a_poll_with_nothing_new_reads_only_the_index_a_replaced_run_is_read_again() {
    let mut fake = Fake::new(&[run(json!({}))]);
    let mut tab = started(&mut fake);
    let before = fake.calls_to("data.read").len();
    let drawn = root(&tab);
    fake.poll(&mut tab);
    assert_eq!(fake.calls_to("data.read").len(), before + 1, "the index");
    assert_eq!(root(&tab), drawn, "and nothing drawn again");
    let newer = passed(json!({ "at": now() + 1000 }));
    fake.set_runs(&[newer]);
    fake.poll(&mut tab);
    assert_eq!(tab.kit().runs()[0].status, logic::Status::Passed);
    assert_eq!(
        last_badge(&fake),
        json!({ "state": "success", "text": "OK" })
    );
    assert_eq!(
        fake.calls_to("data.read").len(),
        before + 3,
        "the index, the index and the run"
    );
}

#[test]
fn the_poll_is_every_three_seconds_and_the_tab_hears_the_panes_and_the_theme() {
    let mut fake = Fake::new(&[]);
    let tab = started(&mut fake);
    let timers: Vec<(&str, u32)> = tab
        .subscriptions()
        .timers
        .iter()
        .map(|timer| (timer.id.as_str(), timer.every_ms))
        .collect();
    assert_eq!(timers, vec![("poll", 3000)]);
    assert!(
        tab.subscriptions()
            .events
            .contains(&"panes.changed".to_owned())
    );
    assert!(tab.subscriptions().events.contains(&"theme".to_owned()));
    assert!(tab.subscriptions().shortcuts.is_empty());
}

#[test]
fn a_poll_still_going_is_not_started_again() {
    let mut fake = Fake::new(&[run(json!({}))]);
    let mut tab = started(&mut fake);
    tab.timer("poll");
    tab.timer("poll");
    assert_eq!(
        tab.methods(),
        vec!["data.read"],
        "one read of the index waits, not two"
    );
    fake.pump(&mut tab);
    tab.timer("poll");
    assert_eq!(
        tab.methods(),
        vec!["data.read"],
        "and once it is done, the next one is"
    );
}

/// no run yet is an ordinary state, not an error
#[test]
fn no_run_yet_is_an_ordinary_state_not_an_error() {
    let mut fake = Fake::new(&[]);
    let mut tab = started(&mut fake);
    fake.files.remove("index.json");
    fake.poll(&mut tab);
    assert_eq!(tab.kit().screen(), Screen::List);
    assert!(tab.kit().runs().is_empty());
    assert_eq!(last_badge(&fake), json!({ "state": null }), "no badge");
}

#[test]
fn a_tab_started_before_the_hook_ever_ran_has_no_index_and_says_so() {
    let mut fake = Fake::new(&[]);
    fake.files.clear();
    let tab = started(&mut fake);
    assert_eq!(tab.kit().screen(), Screen::List);
    assert!(tab.shows("No test run yet"));
}

/// denied: no data.read, said as a state, with no badge, and a way to try again
#[test]
fn denied_no_data_read_said_as_a_state_with_no_badge_and_a_way_to_try_again() {
    let mut fake = Fake::new(&[run(json!({}))]);
    fake.deny_read = true;
    let mut tab = started(&mut fake);
    assert_eq!(tab.kit().screen(), Screen::Denied);
    assert_eq!(last_badge(&fake), json!({ "state": null }));
    fake.log.clear();
    tab.press("Try again");
    fake.pump(&mut tab);
    assert_eq!(fake.calls_to("data.read").len(), 1, "retry reads again");
    assert_eq!(
        fake.calls_to("panes.list").len(),
        1,
        "and lists the agents again"
    );
    assert_eq!(tab.kit().screen(), Screen::Denied);
}

#[test]
fn a_permission_given_later_is_seen_by_the_next_poll() {
    let mut fake = Fake::new(&[run(json!({}))]);
    fake.deny_read = true;
    let mut tab = started(&mut fake);
    assert_eq!(tab.kit().screen(), Screen::Denied);
    fake.deny_read = false;
    fake.poll(&mut tab);
    assert_eq!(tab.kit().screen(), Screen::List);
    assert_eq!(tab.kit().runs().len(), 1);
    assert_eq!(
        last_badge(&fake),
        json!({ "state": "failure", "text": "2 failed" })
    );
}

/// a read that fails is an error, and what was listed stays, marked stale, when a later read fails
#[test]
fn a_read_that_fails_is_an_error_and_an_index_nobody_can_read_lists_nothing() {
    let mut fake = Fake::new(&[run(json!({}))]);
    fake.read_error = Some("failed".into());
    let tab = started(&mut fake);
    assert_eq!(tab.kit().screen(), Screen::Error);

    let mut fake = Fake::new(&[run(json!({}))]);
    let mut tab = started(&mut fake);
    fake.files.insert("index.json".into(), "{".into());
    fake.poll(&mut tab);
    assert_eq!(tab.kit().screen(), Screen::List);
    assert!(
        tab.kit().runs().is_empty(),
        "an index nobody can read lists nothing"
    );
}

#[test]
fn a_later_read_that_fails_keeps_what_was_listed_marked_stale_until_one_goes_through() {
    let mut fake = Fake::new(&[run(json!({}))]);
    let mut tab = started(&mut fake);
    fake.read_error = Some("failed".into());
    fake.poll(&mut tab);
    assert_eq!(tab.kit().screen(), Screen::List);
    assert_eq!(tab.kit().runs().len(), 1, "what is listed stays");
    assert!(tab.shows("The last read failed. Trying again…"));
    assert!(tab.shows("math > adds"));
    fake.read_error = None;
    fake.poll(&mut tab);
    assert!(!tab.shows("The last read failed"));
}

/// a run file that is missing or broken is skipped, not fatal
#[test]
fn a_run_file_that_is_missing_or_broken_is_skipped_not_fatal() {
    let runs = [
        run(json!({})),
        run(json!({ "key": KEY_B, "project": "other" })),
        run(json!({ "key": KEY_C, "project": "third" })),
    ];
    let mut fake = Fake::new(&runs);
    fake.files.remove(&format!("runs/{KEY_B}.json"));
    fake.files
        .insert(format!("runs/{KEY_C}.json"), "{ not a run".into());
    let tab = started(&mut fake);
    let keys: Vec<&str> = tab
        .kit()
        .runs()
        .iter()
        .map(|run| run.key.as_str())
        .collect();
    assert_eq!(keys, vec![KEY_A]);
    assert_eq!(tab.kit().screen(), Screen::List);
}

#[test]
fn a_run_file_refused_midway_is_a_failed_poll_and_the_next_one_reads_it_again() {
    let runs = [
        run(json!({})),
        run(json!({ "key": KEY_B, "project": "other" })),
    ];
    let mut fake = Fake::new(&runs);
    testing::set_now(Some(now()));
    let mut tab = Harness::<Tests>::new(testing::start().grant(["data.read"]));
    // The index goes through, the first run is refused.
    let call = tab
        .calls()
        .iter()
        .find(|call| call.method == "data.read")
        .unwrap()
        .clone();
    tab.reply(call.id, Ok(json!({ "text": fake.files["index.json"] })));
    let call = tab
        .calls()
        .iter()
        .find(|call| call.method == "data.read")
        .unwrap()
        .clone();
    tab.reply(call.id, Err(KitError::new("failed", "disk")));
    assert_eq!(tab.kit().screen(), Screen::Error);
    fake.pump(&mut tab);
    fake.poll(&mut tab);
    assert_eq!(tab.kit().screen(), Screen::List);
    assert_eq!(tab.kit().runs().len(), 2);
}

// ---- handing over ----

/// handing a failure over sends its brief to the agent that ran it, and says so
#[test]
fn handing_a_failure_over_sends_its_brief_to_the_agent_that_ran_it_and_says_so() {
    let mut fake = Fake::new(&[run(json!({}))]);
    let mut tab = started(&mut fake);
    press_nth(&mut tab, HAND_OVER, 1);
    assert_eq!(tab.kit().pending().len(), 1, "it is going");
    fake.pump(&mut tab);
    let sent = sends(&fake);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, "Emma");
    assert_eq!(sent[0].1, brief(&run(json!({})), Some(1)));
    assert!(sent[0].1.chars().count() <= 20_000);
    assert_eq!(
        tab.kit().sent()[&format!("{KEY_A}:1")],
        Sent {
            agent: "Emma".into(),
            run_at: now() - 60_000
        }
    );
    assert!(tab.kit().pending().is_empty());
    press_nth(&mut tab, "Hand over all 2 failures", 0);
    fake.pump(&mut tab);
    let sent = sends(&fake);
    assert_eq!(sent.len(), 2);
    let text = &sent[1].1;
    let adds = text
        .find("Failing test: math > adds")
        .expect("the first failure");
    assert!(
        adds < text
            .find("Failing test: math > throws")
            .expect("the second")
    );
}

/// the person picks another agent, and the brief goes there
#[test]
fn the_person_picks_another_agent_and_the_brief_goes_there() {
    let mut fake = Fake::new(&[run(json!({}))]);
    let mut tab = started(&mut fake);
    tab.choose("Agent", "Jack");
    press_nth(&mut tab, HAND_OVER, 0);
    fake.pump(&mut tab);
    assert_eq!(sends(&fake)[0].0, "Jack");
}

/// nothing is sent with no agent to send to, for a failure that is not there, or twice at once
#[test]
fn nothing_is_sent_with_no_agent_to_send_to() {
    let mut fake = Fake::new(&[run(json!({}))]);
    fake.panes = vec![pane_with("Shell", json!({ "agent": "shell" }))];
    let tab = started(&mut fake);
    let buttons = buttons(&tab, HAND_OVER);
    assert_eq!(buttons.len(), 2);
    assert!(
        buttons.iter().all(|button| button.on_press.is_none()),
        "they are drawn disabled"
    );
    assert!(fake.calls_to("panes.send").is_empty());
}

#[test]
fn nothing_is_sent_for_a_failure_or_a_run_that_is_not_there_any_more() {
    let mut fake = Fake::new(&[run(json!({}))]);
    let mut tab = started(&mut fake);
    // The page as it was drawn, and what its buttons will do when the click arrives.
    let drawn = tab.view().seq;
    let second = handler_of(&tab, HAND_OVER, 1);
    let all = handler_of(&tab, "Hand over all 2 failures", 0);
    // A newer run has only one failure left.
    let one = run(
        json!({ "at": now(), "failures": [failure("math > adds", "no", "src/a.test.js", 4)], "failed": 1 }),
    );
    fake.set_runs(&[one]);
    fake.poll(&mut tab);
    tab.ui_from(drawn, second, None);
    assert!(tab.methods().is_empty(), "the second failure is gone");
    // And then no run at all.
    fake.set_runs(&[]);
    fake.poll(&mut tab);
    tab.ui_from(drawn, all, None);
    assert!(tab.methods().is_empty(), "the run is gone");
    assert!(fake.calls_to("panes.send").is_empty());
}

#[test]
fn a_second_click_while_it_is_going_is_not_a_second_send() {
    let mut fake = Fake::new(&[run(json!({}))]);
    let mut tab = started(&mut fake);
    let drawn = tab.view().seq;
    let first = handler_of(&tab, HAND_OVER, 0);
    tab.ui(first, None);
    tab.ui_from(drawn, first, None);
    assert_eq!(
        tab.methods(),
        vec!["panes.send"],
        "a second click while it is going is not a second send"
    );
    fake.pump(&mut tab);
    assert_eq!(fake.calls_to("panes.send").len(), 1);
}

/// a send that is refused says why, keeps the button, and asks again for the agents when one is gone
#[test]
fn a_send_that_is_refused_says_why_keeps_the_button_and_asks_again_for_the_agents_when_one_is_gone()
{
    for (code, expected, words) in [
        ("denied", "denied", "Accept panes.send in Settings › Kits"),
        ("rate_limited", "rate_limited", "Too many sends in a minute"),
        (
            "not_found",
            "not_found",
            "Emma is not in this workspace any more",
        ),
        ("unavailable", "unavailable", "Emma cannot receive text now"),
        ("boom", "failed", "It could not be sent: could not: boom"),
    ] {
        let mut fake = Fake::new(&[run(json!({}))]);
        fake.send_error = Some(code.into());
        let mut tab = started(&mut fake);
        let listed = fake.calls_to("panes.list").len();
        press_nth(&mut tab, HAND_OVER, 0);
        fake.pump(&mut tab);
        let id = format!("{KEY_A}:0");
        assert_eq!(tab.kit().problems()[&id].code, expected, "{code}");
        assert!(!tab.kit().sent().contains_key(&id), "{code}");
        assert!(tab.shows(words), "{code}");
        assert_eq!(banners(&tab, ui::Notice::Error), 1, "{code}");
        assert!(
            buttons(&tab, HAND_OVER)[0].on_press.is_some(),
            "the button stays, enabled: {code}"
        );
        let expected_lists = if code == "not_found" || code == "unavailable" {
            listed + 1
        } else {
            listed
        };
        assert_eq!(fake.calls_to("panes.list").len(), expected_lists, "{code}");
        // Trying again forgets the refusal while it goes.
        fake.send_error = None;
        press_nth(&mut tab, HAND_OVER, 0);
        assert!(tab.kit().problems().is_empty(), "{code}");
        fake.pump(&mut tab);
        assert!(tab.kit().sent().contains_key(&id), "{code}");
    }
}

/// without panes.read the tab still lists the runs, and says what is missing
#[test]
fn without_panes_read_the_tab_still_lists_the_runs_and_says_what_is_missing() {
    let mut fake = Fake::new(&[run(json!({}))]);
    fake.deny_panes = true;
    let tab = started(&mut fake);
    assert_eq!(tab.kit().screen(), Screen::List);
    assert!(tab.kit().panes_denied());
    assert!(tab.kit().panes().is_empty());
    assert!(tab.shows("Accept panes.read in Settings › Kits"));
}

/// the agents are listed again when the panes change
#[test]
fn the_agents_are_listed_again_when_the_panes_change() {
    let mut fake = Fake::new(&[run(json!({}))]);
    let mut tab = started(&mut fake);
    fake.panes = vec![pane("Zed")];
    tab.event("panes.changed", json!({}));
    fake.pump(&mut tab);
    let names: Vec<&str> = tab
        .kit()
        .panes()
        .iter()
        .map(|pane| pane.name.as_str())
        .collect();
    assert_eq!(names, vec!["Zed"]);
    assert!(tab.shows("Agent: Zed"));
}

/// a hand-over is forgotten when its run is replaced by a newer one
#[test]
fn a_hand_over_is_forgotten_when_its_run_is_replaced_by_a_newer_one() {
    let mut fake = Fake::new(&[run(json!({}))]);
    let mut tab = started(&mut fake);
    press_nth(&mut tab, HAND_OVER, 0);
    fake.pump(&mut tab);
    let id = format!("{KEY_A}:0");
    assert!(tab.kit().sent().contains_key(&id));
    assert!(tab.shows("Handed to Emma"));
    fake.set_runs(&[run(json!({ "at": now() + 5000 }))]);
    fake.poll(&mut tab);
    assert!(!tab.kit().sent().contains_key(&id));
    assert!(!tab.shows("Handed to Emma"));
}

#[test]
fn a_refusal_is_forgotten_when_its_run_is_gone() {
    let mut fake = Fake::new(&[run(json!({}))]);
    fake.send_error = Some("rate_limited".into());
    let mut tab = started(&mut fake);
    press_nth(&mut tab, HAND_OVER, 0);
    fake.pump(&mut tab);
    assert_eq!(tab.kit().problems().len(), 1);
    fake.set_runs(&[]);
    fake.poll(&mut tab);
    assert!(tab.kit().problems().is_empty());
}

#[test]
fn the_clipboard_gets_the_command_and_the_message_and_the_person_is_told() {
    let mut fake = Fake::new(&[run(json!({}))]);
    let mut tab = started(&mut fake);
    press_nth(&mut tab, "Copy the command", 0);
    fake.pump(&mut tab);
    assert_eq!(
        fake.calls_to("ui.copy")[0].params,
        json!({ "text": "pnpm vitest run" })
    );
    assert_eq!(fake.calls_to("ui.toast")[0].params["message"], "Copied");
    press_nth(&mut tab, "Copy the message", 1);
    fake.pump(&mut tab);
    assert_eq!(
        fake.calls_to("ui.copy")[1].params,
        json!({ "text": "Error: kaboom" })
    );
}

/// the tab only calls what it holds
#[test]
fn the_tab_only_calls_what_it_holds() {
    let mut fake = Fake::new(&[
        run(json!({})),
        passed(json!({ "key": KEY_B, "project": "other" })),
    ]);
    fake.send_error = Some("denied".into());
    let mut tab = started(&mut fake);
    press_nth(&mut tab, HAND_OVER, 0);
    press_nth(&mut tab, "Copy the command", 0);
    fake.pump(&mut tab);
    tab.event("theme", json!({ "scheme": "dark", "locale": "fr" }));
    tab.event("panes.changed", json!({}));
    fake.poll(&mut tab);
    let mut methods: Vec<&str> = fake.log.iter().map(|call| call.method.as_str()).collect();
    methods.sort();
    methods.dedup();
    assert_eq!(
        methods,
        vec![
            "data.read",
            "panes.list",
            "panes.send",
            "tab.setBadge",
            "tab.setTitle",
            "ui.copy",
            "ui.toast"
        ],
        "data.read, panes.read and panes.send are what the manifest asks for; the rest needs no permission"
    );
}

// ---- the view ----

fn base() -> Vec<Value> {
    vec![run(json!({}))]
}

/// a failing run: each failure is a row with its message and its own button
#[test]
fn a_failing_run_each_failure_is_a_row_with_its_message_and_its_own_button() {
    let mut fake = Fake::new(&base());
    let mut tab = started(&mut fake);
    for words in [
        "Tests",
        "2 failed",
        "math > adds",
        "src/b.test.js:9",
        "Error: kaboom",
        "pnpm vitest run",
        "Emma · vitest",
        "2 failed · 10 passed · 1 skipped · 2.3\u{a0}s · 1\u{a0}min ago",
    ] {
        assert!(tab.shows(words), "{words:?} in {:#?}", tab.texts());
    }
    assert_eq!(buttons(&tab, HAND_OVER).len(), 2);
    press_nth(&mut tab, HAND_OVER, 1);
    fake.pump(&mut tab);
    assert_eq!(sends(&fake)[0].1, brief(&run(json!({})), Some(1)));
    assert_eq!(
        states(&tab)
            .into_iter()
            .filter(|status| *status == ui::Status::Failure)
            .count(),
        1,
        "the header says it as a state, not a coloured page"
    );
    assert_eq!(buttons(&tab, "Hand over all 2 failures").len(), 1);
}

/// a message is shown as text, whatever it holds
#[test]
fn a_message_is_shown_as_text_whatever_it_holds() {
    let hostile = failure(
        "<img src=x onerror=alert(1)>",
        "<script>alert(1)</script> [link](javascript:1)",
        "src/a.test.js",
        4,
    );
    let mut fake = Fake::new(&[run(json!({ "failures": [hostile] }))]);
    let tab = started(&mut fake);
    assert!(tab.shows("<script>alert(1)</script> [link](javascript:1)"));
    assert!(tab.shows("<img src=x onerror=alert(1)>"));
    let links = collect(&tab, |element| match element {
        Node::Link(_) | Node::Spans(_) | Node::Image(_) => Some(()),
        _ => None,
    });
    assert!(
        links.is_empty(),
        "nothing in a message becomes a link or an image"
    );
}

/// the picker: one agent is named, several are chosen from, none is said
#[test]
fn the_picker_one_agent_is_named_several_are_chosen_from_none_is_said() {
    let mut fake = Fake::new(&base());
    let mut tab = started(&mut fake);
    let several = select(&tab).expect("a choice of agents");
    let names: Vec<&str> = several
        .options
        .iter()
        .map(|option| option.label.as_str())
        .collect();
    assert_eq!(names, vec!["Emma", "Jack"]);
    assert_eq!(several.selected.as_deref(), Some("Emma"));
    tab.choose("Agent", "Jack");
    assert_eq!(select(&tab).unwrap().selected.as_deref(), Some("Jack"));

    let mut fake = Fake::new(&base());
    fake.panes = vec![pane("Emma")];
    let one = started(&mut fake);
    assert!(one.shows("Agent: Emma"));
    assert!(select(&one).is_none());

    let mut fake = Fake::new(&base());
    fake.panes = vec![];
    let none = started(&mut fake);
    assert!(none.shows("No Claude Code agent can receive text now."));
    assert!(
        buttons(&none, HAND_OVER)
            .iter()
            .all(|button| button.on_press.is_none())
    );

    let mut fake = Fake::new(&base());
    fake.deny_panes = true;
    assert!(started(&mut fake).shows("Accept panes.read in Settings › Kits"));
}

/// sending, sent and refused
#[test]
fn sending_sent_and_refused() {
    let mut fake = Fake::new(&base());
    let mut tab = started(&mut fake);
    press_nth(&mut tab, HAND_OVER, 0);
    let sending = buttons(&tab, "Sending…");
    assert!(
        sending.len() == 1 && sending[0].on_press.is_none(),
        "disabled while it goes"
    );
    fake.pump(&mut tab);
    assert!(tab.shows("Handed to Emma"));
    assert!(states(&tab).contains(&ui::Status::Success));
    assert_eq!(
        buttons(&tab, HAND_OVER).len(),
        1,
        "the other failure still has its button"
    );

    let mut fake = Fake::new(&base());
    fake.send_error = Some("unavailable".into());
    let mut tab = started(&mut fake);
    press_nth(&mut tab, HAND_OVER, 1);
    fake.pump(&mut tab);
    assert!(tab.shows("Emma cannot receive text now"));
    assert_eq!(banners(&tab, ui::Notice::Error), 1);
}

/// all green, with what ran
#[test]
fn all_green_with_what_ran() {
    let green = passed(json!({ "passed": 2224, "skipped": 0, "duration": 53250 }));
    let mut fake = Fake::new(&[green]);
    let tab = started(&mut fake);
    assert!(tab.shows("All green"));
    assert!(tab.shows("2224 passed"));
    assert_eq!(
        states(&tab)
            .iter()
            .filter(|status| **status == ui::Status::Success)
            .count(),
        1
    );
    assert!(buttons(&tab, HAND_OVER).is_empty(), "nothing to hand over");
    assert!(select(&tab).is_none());
    assert!(tab.shows("All green · 2224 passed · 53\u{a0}s"));
}

/// not understood is said, and is not green
#[test]
fn not_understood_is_said_and_is_not_green() {
    let unknown = run(json!({
        "status": "not-understood", "failed": null, "passed": null, "skipped": null,
        "duration": null, "failures": [], "framework": null, "command": "pnpm test",
    }));
    let mut fake = Fake::new(std::slice::from_ref(&unknown));
    let tab = started(&mut fake);
    assert!(tab.shows(
        "Not understood. The output of this command has no test results Hangar can read, so nothing is assumed to have passed."
    ));
    assert_eq!(
        states(&tab),
        vec![ui::Status::Neutral],
        "the header, not green"
    );
    assert_eq!(glyphs(&tab), vec![ui::Status::Neutral]);
    assert!(!tab.shows("All green") && !tab.shows("Passed"));
    assert!(tab.shows("pnpm test"));
    assert!(tab.shows("Emma"), "the title without a framework");

    let mut fake = Fake::new(&[run(json!({
        "status": "not-understood", "reason": "no-tests", "failed": null, "failures": [],
    }))]);
    assert!(started(&mut fake).shows("reports no test at all"));
}

/// a cut output and a summary with no detail say so
#[test]
fn a_cut_output_and_a_summary_with_no_detail_say_so() {
    let page = |overrides: Value| {
        let mut fake = Fake::new(&[run(overrides)]);
        started(&mut fake)
    };
    assert!(
        page(json!({ "partial": true, "passed": null, "skipped": null, "failed": 2 }))
            .shows("cut before its summary")
    );
    let summary_only = page(json!({ "failures": [], "failed": 4 }));
    assert!(summary_only.shows("shows no detail for these failures"));
    assert!(
        buttons(&summary_only, HAND_OVER).is_empty(),
        "nothing to hand over without a failure"
    );
    assert!(page(json!({ "failed": 60 })).shows("Showing 2 of 60."));
    assert!(
        page(json!({ "status": "failed", "failed": 0, "failures": [], "exitCode": 2 }))
            .shows("exited with code 2")
    );
    assert!(
        !page(json!({ "failed": 0, "failures": [], "exitCode": null })).shows("exited with code")
    );
}

/// no run yet, denied, error and loading are designed states
#[test]
fn no_run_yet_denied_error_and_loading_are_designed_states() {
    let kind = |tab: &Harness<Tests>| {
        collect(tab, |element| match element {
            Node::Empty(empty) => Some(empty.kind),
            _ => None,
        })
    };

    let mut fake = Fake::new(&[]);
    let none = started(&mut fake);
    assert!(none.shows("No test run yet"));
    assert!(none.shows("vitest, jest, pytest, cargo test, go test, node --test, npm test"));
    assert!(none.shows("No run yet"), "the header too");
    assert_eq!(kind(&none), vec![ui::EmptyKind::Empty]);

    let mut fake = Fake::new(&base());
    fake.deny_read = true;
    let denied = started(&mut fake);
    assert!(denied.shows("This tab may not read the kit’s data"));
    assert!(denied.shows("Accept data.read in Settings › Kits"));
    assert_eq!(kind(&denied), vec![ui::EmptyKind::Denied]);
    let retry = buttons(&denied, "Try again");
    assert_eq!(retry.len(), 1);
    assert_eq!(retry[0].kind, ui::ButtonKind::Default);

    let mut fake = Fake::new(&base());
    fake.read_error = Some("failed".into());
    let error = started(&mut fake);
    assert_eq!(kind(&error), vec![ui::EmptyKind::Error]);
    assert_eq!(
        buttons(&error, "Try again")[0].kind,
        ui::ButtonKind::Primary
    );
    assert!(error.shows("The test runs could not be read"));

    testing::set_now(Some(now()));
    let loading = Harness::<Tests>::new(testing::start().grant(["data.read"]));
    assert!(loading.shows("Reading the test runs…"));
    assert_eq!(kind(&loading), vec![ui::EmptyKind::Loading]);
    assert_eq!(loading.kit().screen(), Screen::Loading);
}

/// French reads with its accents and its words
#[test]
fn french_reads_with_its_accents_and_its_words() {
    let mut fake = Fake::new(&base());
    let tab = started_as(&mut fake, testing::start().locale("fr"));
    assert!(tab.shows("2 échecs"));
    assert!(tab.shows("Confier à un agent"));
    assert!(tab.shows("Confier les 2 échecs"));
    assert!(tab.shows("2 échecs · 10 réussis · 1 ignoré"));
    assert!(tab.shows("il y a 1\u{a0}min"));

    let mut fake = Fake::new(&base());
    fake.deny_read = true;
    assert!(
        started_as(&mut fake, testing::start().locale("fr"))
            .shows("Acceptez data.read dans Réglages › Kits")
    );
    let mut fake = Fake::new(&[]);
    assert!(
        started_as(&mut fake, testing::start().locale("fr")).shows("Aucune exécution de tests")
    );
    let mut fake = Fake::new(&base());
    fake.send_error = Some("denied".into());
    let mut tab = started_as(&mut fake, testing::start().locale("fr"));
    press_nth(&mut tab, "Confier à un agent", 0);
    fake.pump(&mut tab);
    assert!(tab.shows("Cet onglet n’a pas le droit d’écrire aux agents"));
}

/// the time updates without drawing again
#[test]
fn the_time_updates_on_the_next_poll() {
    let mut fake = Fake::new(&base());
    let mut tab = started(&mut fake);
    assert!(tab.shows("1\u{a0}min ago"));
    fake.poll(&mut tab);
    assert!(tab.shows("1\u{a0}min ago"));
    testing::set_now(Some(now() + 5 * 60_000));
    fake.poll(&mut tab);
    assert!(tab.shows("6\u{a0}min ago"));
    assert_eq!(
        fake.calls_to("data.read").len(),
        4,
        "and only the index was read again"
    );
}

#[test]
fn an_older_run_says_when_in_the_persons_time_zone() {
    let mut fake = Fake::new(&[run(json!({ "at": now() - 2 * 24 * 3_600_000 }))]);
    let tab = started_as(&mut fake, testing::start().utc_offset(60));
    assert!(tab.shows("Oct 29 at 6:02\u{202f}PM"), "{:#?}", tab.texts());
}

/// several projects get a section each, the failing one first
#[test]
fn several_projects_get_a_section_each_the_failing_one_first() {
    let runs = [
        passed(json!({ "key": KEY_B, "project": "green-app", "at": now() })),
        run(json!({})),
    ];
    let mut fake = Fake::new(&runs);
    let tab = started(&mut fake);
    assert_eq!(section_titles(&tab), vec!["app", "green-app"]);
    assert!(tab.shows("2 failed"));
}

// ---- the trap: a view stays under 4 MB ----

fn heavy_runs() -> Vec<Value> {
    (0..40)
        .map(|n| {
            let key = format!("{n:016x}");
            let failures: Vec<Value> = (0..50)
                .map(|f| failure(&format!("suite {n} > test {f}"), &"x".repeat(2000), "src/a.test.js", f + 1))
                .collect();
            run(json!({ "key": key, "project": format!("project {n}"), "agent": "Emma", "failures": failures, "failed": 50 }))
        })
        .collect()
}

/// 40 runs x 50 failures x 2 000 characters is about 4 MB: a view folds each run's failures past
/// the first five
#[test]
fn forty_runs_of_fifty_failures_stay_far_under_the_4_mb_a_view_may_be() {
    let mut fake = Fake::new(&heavy_runs());
    let mut tab = started(&mut fake);
    assert_eq!(tab.kit().runs().len(), 40);
    let size = serde_json::to_vec(tab.view()).unwrap().len();
    assert!(size < 1_000_000, "a view of {size} bytes");
    assert_eq!(buttons(&tab, HAND_OVER).len(), 40 * 5);
    assert_eq!(buttons(&tab, "Show 45 more").len(), 40);
    assert_eq!(
        buttons(&tab, "Hand over all 50 failures").len(),
        40,
        "all of them can still be handed over"
    );

    // Showing a run's failures is the person's call, one run at a time, and can be taken back.
    press_nth(&mut tab, "Show 45 more", 0);
    assert_eq!(buttons(&tab, HAND_OVER).len(), 39 * 5 + 50);
    assert_eq!(buttons(&tab, "Show 45 more").len(), 39);
    assert_eq!(buttons(&tab, "Show fewer").len(), 1);
    press_nth(&mut tab, "Show fewer", 0);
    assert_eq!(buttons(&tab, HAND_OVER).len(), 40 * 5);

    // What is handed over does not depend on what is shown.
    press_nth(&mut tab, "Hand over all 50 failures", 0);
    fake.pump(&mut tab);
    let (_, text) = sends(&fake).remove(0);
    assert_eq!(text.matches("Failing test: ").count(), 5);
    assert!(
        text.ends_with(
            "45 more failing test(s) are in the full output: run the command to see them."
        )
    );
}

#[test]
fn a_run_with_five_failures_or_fewer_has_nothing_to_fold() {
    let five: Vec<Value> = (0..5)
        .map(|f| failure(&format!("t{f}"), "no", "a.js", 1))
        .collect();
    let mut fake = Fake::new(&[run(json!({ "failures": five, "failed": 5 }))]);
    let tab = started(&mut fake);
    assert_eq!(buttons(&tab, HAND_OVER).len(), 5);
    assert!(!tab.texts().iter().any(|text| text.starts_with("Show ")));
}
