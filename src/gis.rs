//! Thin wrapper around the `gisRequestTokenPromise` JS function defined in
//! `index.html`, which drives Google Identity Services' browser-native
//! OAuth token flow. No client_secret is used anywhere in this app.

use wasm_bindgen::prelude::*;

pub const DRIVE_APPDATA_SCOPE: &str = "https://www.googleapis.com/auth/drive.appdata";

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = gisRequestTokenPromise)]
    fn gis_request_token_promise(client_id: &str, scope: &str) -> js_sys::Promise;
}

/// Opens the Google sign-in popup and resolves with a short-lived
/// (~1 hour) OAuth access token scoped to drive.appdata.
pub async fn request_access_token(client_id: &str) -> Result<String, String> {
    let promise = gis_request_token_promise(client_id, DRIVE_APPDATA_SCOPE);
    let result = wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|e| e.as_string().unwrap_or_else(|| format!("{e:?}")))?;
    result
        .as_string()
        .ok_or_else(|| "Unexpected response from Google sign-in".to_string())
}
