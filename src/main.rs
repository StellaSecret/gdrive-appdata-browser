mod browser;
mod drive;
mod gis;

use dioxus::prelude::*;
use drive::{DriveFile, DriveRevision};
use std::collections::HashMap;

fn main() {
    dioxus::launch(App);
}

/// Quick-sign-in presets for specific projects you personally control,
/// baked in at build time from their own individually-named GitHub Actions
/// secrets (see `.github/workflows/pipeline.yml`). Each entry is
/// deliberately per-project, not a shared/admin identity - `appDataFolder`
/// access is scoped per OAuth app, so a preset here only ever unlocks the
/// one project it belongs to.
///
/// To add another: create a new repo secret, add a line here, and wire the
/// same secret name into the `build` job's env block.
///
/// Missing/unset secrets resolve to `Some("")` (GitHub Actions substitutes
/// an empty string for a `secrets.X` reference that doesn't exist) or
/// `None` (env var truly absent, e.g. local builds) - `filter_available_presets`
/// treats both the same way and simply omits that preset's button.
const KNOWN_PROJECTS: &[(&str, Option<&str>)] = &[
    (
        "PeopleModeler",
        option_env!("GOOGLE_WEB_CLIENT_ID_PEOPLEMODELER"),
    ),
    (
        "AsthmaTrack",
        option_env!("GOOGLE_WEB_CLIENT_ID_ASTHMATRACK"),
    ),
];

// Preset icons, bundled via Dioxus's asset! macro (see
// assets/icons/README.md for provenance). Optional: a preset without a
// matching entry here just falls back to the colored-initials badge.
const PEOPLEMODELER_ICON: Asset = asset!("/assets/icons/peoplemodeler.svg");
const ASTHMATRACK_ICON: Asset = asset!("/assets/icons/asthmatrack.png");

fn preset_icon(name: &str) -> Option<Asset> {
    match name {
        "PeopleModeler" => Some(PEOPLEMODELER_ICON),
        "AsthmaTrack" => Some(ASTHMATRACK_ICON),
        _ => None,
    }
}

fn filter_available_presets(
    raw: &[(&'static str, Option<&'static str>)],
) -> Vec<(&'static str, &'static str)> {
    raw.iter()
        .filter_map(|(name, id)| id.filter(|s| !s.is_empty()).map(|id| (*name, id)))
        .collect()
}

/// Deterministic avatar background color for a preset button, derived from
/// the project name so the same project always gets the same color. Used
/// as the fallback badge for any preset without a dedicated icon asset.
fn avatar_color(name: &str) -> String {
    let hue = name.bytes().map(|b| b as u32).sum::<u32>() % 360;
    format!("hsl({hue}, 55%, 42%)")
}

fn avatar_initials(name: &str) -> String {
    name.chars().take(2).collect::<String>().to_uppercase()
}

#[component]
fn App() -> Element {
    let mut client_id = use_signal(|| browser::load_saved_client_id().unwrap_or_default());
    let mut access_token: Signal<Option<String>> = use_signal(|| None);
    let mut status: Signal<Option<String>> = use_signal(|| None);
    let mut signing_in = use_signal(|| false);

    let mut files: Signal<Vec<DriveFile>> = use_signal(Vec::new);
    let mut revisions: Signal<HashMap<String, Vec<DriveRevision>>> = use_signal(HashMap::new);
    let mut loading_files = use_signal(|| false);

    let mut begin_sign_in = move |id: String| {
        browser::save_client_id(&id);
        signing_in.set(true);
        status.set(None);

        spawn(async move {
            match gis::request_access_token(&id).await {
                Ok(token) => {
                    access_token.set(Some(token.clone()));
                    signing_in.set(false);
                    loading_files.set(true);
                    match drive::list_appdata_files(&token).await {
                        Ok(f) => {
                            files.set(f);
                            loading_files.set(false);
                        }
                        Err(e) => {
                            status.set(Some(format!("Failed to list files: {e}")));
                            loading_files.set(false);
                        }
                    }
                }
                Err(e) => {
                    status.set(Some(format!("Sign-in failed: {e}")));
                    signing_in.set(false);
                }
            }
        });
    };

    let do_sign_in = move |_| {
        let id = client_id.read().trim().to_string();
        if id.is_empty() {
            status.set(Some("Enter your Google OAuth Client ID first.".to_string()));
            return;
        }
        begin_sign_in(id);
    };

    let presets = filter_available_presets(KNOWN_PROJECTS);

    rsx! {
        style { {include_str!("./style.css")} }

        div { class: "page",
            header {
                h1 { "appDataFolder Browser" }
                p { class: "muted", "Runs entirely in your browser. Nothing but your own requests to Google ever leaves this page." }
            }

            main {
                if access_token.read().is_none() {
                    div { class: "card",
                        if !presets.is_empty() {
                            div { class: "presets",
                                span { class: "muted preset-label", "Quick sign-in:" }
                                for (name , preset_id) in presets {
                                    button {
                                        key: "{name}",
                                        class: "preset-avatar",
                                        style: if preset_icon(name).is_none() { "background: {avatar_color(name)}" },
                                        title: "Sign in with {name}'s Client ID",
                                        disabled: *signing_in.read(),
                                        onclick: move |_| {
                                            client_id.set(preset_id.to_string());
                                            begin_sign_in(preset_id.to_string());
                                        },
                                        if let Some(icon) = preset_icon(name) {
                                            img { class: "preset-icon-img", src: "{icon}", alt: "{name}" }
                                        } else {
                                            "{avatar_initials(name)}"
                                        }
                                    }
                                }
                            }
                        }

                        label { r#for: "client-id", "Google OAuth Client ID" }
                        input {
                            id: "client-id",
                            r#type: "text",
                            placeholder: "xxxxxxxx.apps.googleusercontent.com",
                            value: "{client_id}",
                            oninput: move |evt| client_id.set(evt.value()),
                        }
                        p { class: "hint",
                            "Create a "
                            strong { "Web application" }
                            " type OAuth Client ID in Google Cloud Console, with this page's URL added as an authorized JavaScript origin. Only the Client ID is needed here — never a client secret."
                        }
                        button {
                            class: "primary",
                            disabled: *signing_in.read(),
                            onclick: do_sign_in,
                            if *signing_in.read() { "Opening sign-in…" } else { "Sign in with Google" }
                        }
                        if let Some(msg) = status.read().clone() {
                            p { class: "error", "{msg}" }
                        }
                    }
                } else {
                    div { class: "toolbar",
                        span { class: "muted", "Signed in · token active for ~1 hour" }
                        button {
                            onclick: move |_| {
                                access_token.set(None);
                                files.set(Vec::new());
                                revisions.set(HashMap::new());
                            },
                            "Sign out"
                        }
                    }

                    if *loading_files.read() {
                        div { class: "empty", "Loading files…" }
                    } else if files.read().is_empty() {
                        div { class: "empty", "No files found in appDataFolder for this account." }
                    } else {
                        for file in files.read().iter().cloned() {
                            FileCard {
                                file: file.clone(),
                                token: access_token.read().clone().unwrap_or_default(),
                                revisions,
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn FileCard(
    file: DriveFile,
    token: String,
    revisions: Signal<HashMap<String, Vec<DriveRevision>>>,
) -> Element {
    let file_id = file.id.clone();
    let mut loading = use_signal(|| true);
    let mut error = use_signal(|| None::<String>);

    {
        let file_id = file_id.clone();
        let token = token.clone();
        use_effect(move || {
            let file_id = file_id.clone();
            let token = token.clone();
            spawn(async move {
                match drive::list_revisions(&token, &file_id).await {
                    Ok(revs) => {
                        revisions.write().insert(file_id, revs);
                        loading.set(false);
                    }
                    Err(e) => {
                        error.set(Some(e));
                        loading.set(false);
                    }
                }
            });
        });
    }

    let revs = revisions.read().get(&file.id).cloned().unwrap_or_default();

    rsx! {
        div { class: "file-card",
            div { class: "file-header",
                div {
                    div { class: "name", "{file.name}" }
                    div { class: "id", "{file.id}" }
                }
                div { class: "muted",
                    "modified {file.modified_time.clone().unwrap_or_default()} · {fmt_size(&file.size)}"
                }
            }

            if *loading.read() {
                div { class: "empty", "Loading revisions…" }
            } else if let Some(err) = error.read().clone() {
                div { class: "error", "Failed to load revisions: {err}" }
            } else if revs.is_empty() {
                div { class: "empty", "No revisions available." }
            } else {
                table {
                    thead {
                        tr {
                            th { "Revision ID" }
                            th { "Modified" }
                            th { "Size" }
                            th { "Actions" }
                        }
                    }
                    tbody {
                        for rev in revs.iter().cloned() {
                            RevisionRow {
                                file_id: file.id.clone(),
                                revision: rev,
                                token: token.clone(),
                                revisions,
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn RevisionRow(
    file_id: String,
    revision: DriveRevision,
    token: String,
    revisions: Signal<HashMap<String, Vec<DriveRevision>>>,
) -> Element {
    let mut busy = use_signal(|| false);
    let mut row_error = use_signal(|| None::<String>);

    let rev_id = revision.revision_id_or_id();

    let download = {
        let file_id = file_id.clone();
        let token = token.clone();
        let rev_id = rev_id.clone();
        move |_| {
            let file_id = file_id.clone();
            let token = token.clone();
            let rev_id = rev_id.clone();
            spawn(async move {
                match drive::download_revision(&token, &file_id, &rev_id).await {
                    Ok(bytes) => {
                        browser::trigger_download(&format!("revision_{rev_id}.json"), &bytes);
                    }
                    Err(e) => row_error.set(Some(e)),
                }
            });
        }
    };

    let delete = {
        let file_id = file_id.clone();
        let token = token.clone();
        let rev_id = rev_id.clone();
        move |_| {
            if !browser::confirm("Permanently delete this revision? This cannot be undone.") {
                return;
            }
            let file_id = file_id.clone();
            let token = token.clone();
            let rev_id = rev_id.clone();
            busy.set(true);
            spawn(async move {
                match drive::delete_revision(&token, &file_id, &rev_id).await {
                    Ok(()) => {
                        if let Some(list) = revisions.write().get_mut(&file_id) {
                            list.retain(|r| r.id != rev_id);
                        }
                    }
                    Err(e) => {
                        row_error.set(Some(e));
                        busy.set(false);
                    }
                }
            });
        }
    };

    rsx! {
        tr {
            td { class: "rev-id", "{revision.id}" }
            td { "{revision.modified_time.clone().unwrap_or_default()}" }
            td { "{fmt_size(&revision.size)}" }
            td { class: "actions",
                button { onclick: download, "Download" }
                button {
                    class: "danger",
                    disabled: *busy.read(),
                    onclick: delete,
                    if *busy.read() { "Deleting…" } else { "Delete" }
                }
                if let Some(err) = row_error.read().clone() {
                    div { class: "error small", "{err}" }
                }
            }
        }
    }
}

impl DriveRevision {
    fn revision_id_or_id(&self) -> String {
        self.id.clone()
    }
}

fn fmt_size(size: &Option<String>) -> String {
    match size.as_ref().and_then(|s| s.parse::<u64>().ok()) {
        Some(n) if n < 1024 => format!("{n} B"),
        Some(n) => format!("{:.1} KB", n as f64 / 1024.0),
        None => "—".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_size_none_shows_dash() {
        assert_eq!(fmt_size(&None), "—");
    }

    #[test]
    fn fmt_size_unparsable_shows_dash() {
        assert_eq!(fmt_size(&Some("not-a-number".to_string())), "—");
    }

    #[test]
    fn fmt_size_bytes_under_1024() {
        assert_eq!(fmt_size(&Some("500".to_string())), "500 B");
        assert_eq!(fmt_size(&Some("0".to_string())), "0 B");
        assert_eq!(fmt_size(&Some("1023".to_string())), "1023 B");
    }

    #[test]
    fn fmt_size_kilobytes_rounds_to_one_decimal() {
        assert_eq!(fmt_size(&Some("1024".to_string())), "1.0 KB");
        assert_eq!(fmt_size(&Some("2048".to_string())), "2.0 KB");
        assert_eq!(fmt_size(&Some("866".to_string())), "866 B");
        assert_eq!(fmt_size(&Some("10240".to_string())), "10.0 KB");
    }

    #[test]
    fn filter_available_presets_skips_none() {
        let raw: &[(&str, Option<&str>)] = &[("NoSecretSet", None)];
        assert_eq!(filter_available_presets(raw), Vec::<(&str, &str)>::new());
    }

    #[test]
    fn filter_available_presets_skips_empty_string() {
        // GitHub Actions substitutes "" for a secrets.X reference that
        // doesn't exist in the repo - must be treated the same as None.
        let raw: &[(&str, Option<&str>)] = &[("UnsetSecret", Some(""))];
        assert_eq!(filter_available_presets(raw), Vec::<(&str, &str)>::new());
    }

    #[test]
    fn filter_available_presets_keeps_configured_ones() {
        let raw: &[(&str, Option<&str>)] = &[
            ("PeopleModeler", Some("123.apps.googleusercontent.com")),
            ("NotConfigured", None),
            ("AsthmaTrack", Some("456.apps.googleusercontent.com")),
            ("AlsoUnset", Some("")),
        ];
        assert_eq!(
            filter_available_presets(raw),
            vec![
                ("PeopleModeler", "123.apps.googleusercontent.com"),
                ("AsthmaTrack", "456.apps.googleusercontent.com"),
            ]
        );
    }

    #[test]
    fn avatar_color_is_deterministic() {
        assert_eq!(avatar_color("PeopleModeler"), avatar_color("PeopleModeler"));
    }

    #[test]
    fn avatar_color_produces_valid_hsl() {
        let color = avatar_color("AsthmaTrack");
        assert!(color.starts_with("hsl("));
        assert!(color.ends_with(", 55%, 42%)"));
    }

    #[test]
    fn avatar_initials_takes_first_two_chars_uppercased() {
        assert_eq!(avatar_initials("PeopleModeler"), "PE");
        assert_eq!(avatar_initials("AsthmaTrack"), "AS");
        assert_eq!(avatar_initials("x"), "X");
        assert_eq!(avatar_initials(""), "");
    }
}
