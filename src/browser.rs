//! Small helpers for the handful of raw browser APIs we need that Dioxus
//! doesn't wrap itself: triggering a file download, a confirm() dialog,
//! and localStorage for remembering the user's Client ID.

use base64::{engine::general_purpose::STANDARD, Engine};
use wasm_bindgen::JsCast;
use web_sys::{HtmlAnchorElement, HtmlElement};

/// Triggers a browser "Save As" download of `bytes` as `filename`, using a
/// base64 data: URL. Avoids the Blob/ObjectURL APIs entirely for simplicity.
pub fn trigger_download(filename: &str, bytes: &[u8]) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };

    let encoded = STANDARD.encode(bytes);
    let href = format!("data:application/octet-stream;base64,{encoded}");

    let Ok(elem) = document.create_element("a") else {
        return;
    };
    let Ok(anchor) = elem.dyn_into::<HtmlAnchorElement>() else {
        return;
    };
    anchor.set_href(&href);
    anchor.set_download(filename);

    let html_elem: &HtmlElement = anchor.as_ref();
    let _ = document.body().map(|b| b.append_child(&anchor));
    html_elem.click();
    let _ = document.body().map(|b| b.remove_child(&anchor));
}

/// Shows a native browser confirm() dialog. Returns false if unavailable.
pub fn confirm(message: &str) -> bool {
    web_sys::window()
        .and_then(|w| w.confirm_with_message(message).ok())
        .unwrap_or(false)
}

const STORAGE_KEY: &str = "appdata_browser_client_id";

pub fn load_saved_client_id() -> Option<String> {
    web_sys::window()?
        .local_storage()
        .ok()??
        .get_item(STORAGE_KEY)
        .ok()?
}

pub fn save_client_id(client_id: &str) {
    if let Some(storage) = web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
    {
        let _ = storage.set_item(STORAGE_KEY, client_id);
    }
}
