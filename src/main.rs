mod browser;
mod drive;
mod gis;

use dioxus::prelude::*;
use drive::{DriveFile, DriveRevision};
use std::collections::HashMap;

fn main() {
    dioxus::launch(App);
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

    let do_sign_in = move |_| {
        let id = client_id.read().trim().to_string();
        if id.is_empty() {
            status.set(Some("Enter your Google OAuth Client ID first.".to_string()));
            return;
        }
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
}
