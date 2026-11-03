//! Every string of the tab, in English and in French, in
//! the shape `hangar_kit_sdk::i18n::Strings` reads: a count `n` picks `key_one` or `key_other`,
//! English fills in what French lacks, and the typography (no-break spaces) is Hangar's.

use hangar_kit_sdk::i18n::Strings;

pub const EN: &[(&str, &str)] = &[
    ("tab", "Tests"),
    ("loading", "Reading the test runs…"),
    ("stateFailed_one", "{n} failed"),
    ("stateFailed_other", "{n} failed"),
    ("stateGreen", "All green"),
    ("stateUnknown", "Not understood"),
    ("stateNone", "No run yet"),
    ("emptyTitle", "No test run yet"),
    (
        "emptyText",
        "When an agent runs your tests (vitest, jest, pytest, cargo test, go test, node --test, npm test), the result shows here.",
    ),
    ("deniedTitle", "This tab may not read the kit’s data"),
    (
        "deniedText",
        "Accept data.read in Settings › Kits, then try again. The hook keeps recording either way.",
    ),
    ("errorTitle", "The test runs could not be read"),
    (
        "errorText",
        "The hook goes on recording. Try again; if it keeps failing, reload the tab.",
    ),
    ("retry", "Try again"),
    ("staleWarning", "The last read failed. Trying again…"),
    ("runLabel", "{agent} · {framework}"),
    ("runLabelNone", "{agent}"),
    ("failedCount", "{n} failed"),
    ("passedCount", "{n} passed"),
    ("skippedCount", "{n} skipped"),
    ("allGreen", "All green"),
    ("failedLabel", "Failed"),
    ("passedLabel", "Passed"),
    ("unknownLabel", "Not understood"),
    (
        "notUnderstood",
        "Not understood. The output of this command has no test results Hangar can read, so nothing is assumed to have passed.",
    ),
    (
        "noTests",
        "Not understood. This run reports no test at all, so nothing is assumed to have passed.",
    ),
    (
        "partial",
        "The output was cut before its summary: these are the failures seen so far.",
    ),
    ("exitCode", "The command exited with code {code}."),
    (
        "noDetail",
        "The output shows no detail for these failures. Run the command again to see them.",
    ),
    ("showing", "Showing {n} of {total}."),
    ("showMore", "Show {n} more"),
    ("showFewer", "Show fewer"),
    ("handOver", "Hand over to an agent"),
    ("handOverAll", "Hand over all {n} failures"),
    ("handingOver", "Sending…"),
    ("sentTo", "Handed to {agent}"),
    ("agentLabel", "Agent"),
    ("agentIs", "Agent: {name}"),
    ("noAgent", "No Claude Code agent can receive text now."),
    (
        "panesDenied",
        "This tab may not list the agents. Accept panes.read in Settings › Kits to hand a failure over.",
    ),
    (
        "problem.denied",
        "This tab may not send to agents. Accept panes.send in Settings › Kits.",
    ),
    (
        "problem.not_found",
        "{agent} is not in this workspace any more.",
    ),
    (
        "problem.rate_limited",
        "Too many sends in a minute. Try again in a moment.",
    ),
    (
        "problem.unavailable",
        "{agent} cannot receive text now. Pick another agent, or try again.",
    ),
    ("problem.failed", "It could not be sent: {detail}"),
    ("copyCommand", "Copy the command"),
    ("copyMessage", "Copy the message"),
    ("copied", "Copied"),
    ("badgeFailed", "{n} failed"),
    ("badgeOk", "OK"),
];

pub const FR: &[(&str, &str)] = &[
    ("tab", "Tests"),
    ("loading", "Lecture des exécutions de tests…"),
    ("stateFailed_one", "{n} échec"),
    ("stateFailed_other", "{n} échecs"),
    ("stateGreen", "Tout est vert"),
    ("stateUnknown", "Non compris"),
    ("stateNone", "Aucune exécution"),
    ("emptyTitle", "Aucune exécution de tests pour l’instant"),
    (
        "emptyText",
        "Quand un agent lance vos tests (vitest, jest, pytest, cargo test, go test, node --test, npm test), le résultat s’affiche ici.",
    ),
    (
        "deniedTitle",
        "Cet onglet n’a pas le droit de lire les données du kit",
    ),
    (
        "deniedText",
        "Acceptez data.read dans Réglages › Kits, puis réessayez. Le hook enregistre quand même.",
    ),
    (
        "errorTitle",
        "Les exécutions de tests n’ont pas pu être lues",
    ),
    (
        "errorText",
        "Le hook continue d’enregistrer. Réessayez ; si cela persiste, rechargez l’onglet.",
    ),
    ("retry", "Réessayer"),
    (
        "staleWarning",
        "La dernière lecture a échoué. Nouvel essai…",
    ),
    ("runLabel", "{agent} · {framework}"),
    ("runLabelNone", "{agent}"),
    ("failedCount_one", "{n} échec"),
    ("failedCount_other", "{n} échecs"),
    ("passedCount_one", "{n} réussi"),
    ("passedCount_other", "{n} réussis"),
    ("skippedCount_one", "{n} ignoré"),
    ("skippedCount_other", "{n} ignorés"),
    ("allGreen", "Tout est vert"),
    ("failedLabel", "Échoué"),
    ("passedLabel", "Réussi"),
    ("unknownLabel", "Non compris"),
    (
        "notUnderstood",
        "Non compris. La sortie de cette commande ne contient aucun résultat de test que Hangar sache lire : rien n’est considéré comme réussi.",
    ),
    (
        "noTests",
        "Non compris. Cette exécution ne signale aucun test : rien n’est considéré comme réussi.",
    ),
    (
        "partial",
        "La sortie a été coupée avant son résumé : voici les échecs vus jusque-là.",
    ),
    (
        "exitCode",
        "La commande s’est terminée avec le code {code}.",
    ),
    (
        "noDetail",
        "La sortie ne donne aucun détail sur ces échecs. Relancez la commande pour les voir.",
    ),
    ("showing", "{n} affichés sur {total}."),
    ("showMore", "Afficher {n} de plus"),
    ("showFewer", "Afficher moins"),
    ("handOver", "Confier à un agent"),
    ("handOverAll", "Confier les {n} échecs"),
    ("handingOver", "Envoi…"),
    ("sentTo", "Confié à {agent}"),
    ("agentLabel", "Agent"),
    ("agentIs", "Agent : {name}"),
    (
        "noAgent",
        "Aucun agent Claude Code ne peut recevoir de texte pour l’instant.",
    ),
    (
        "panesDenied",
        "Cet onglet n’a pas le droit de lister les agents. Acceptez panes.read dans Réglages › Kits pour confier un échec.",
    ),
    (
        "problem.denied",
        "Cet onglet n’a pas le droit d’écrire aux agents. Acceptez panes.send dans Réglages › Kits.",
    ),
    (
        "problem.not_found",
        "{agent} n’est plus dans cet espace de travail.",
    ),
    (
        "problem.rate_limited",
        "Trop d’envois en une minute. Réessayez dans un instant.",
    ),
    (
        "problem.unavailable",
        "{agent} ne peut pas recevoir de texte pour l’instant. Choisissez un autre agent, ou réessayez.",
    ),
    ("problem.failed", "L’envoi a échoué : {detail}"),
    ("copyCommand", "Copier la commande"),
    ("copyMessage", "Copier le message"),
    ("copied", "Copié"),
    ("badgeFailed", "{n} échecs"),
    ("badgeOk", "OK"),
];

/// The tab's words in `locale` (`en` or `fr`; anything else reads English).
pub fn strings(locale: &str) -> Strings {
    Strings::new(locale).add("en", EN).add("fr", FR)
}
