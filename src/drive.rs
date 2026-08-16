//! Calls to the Google Drive REST API directly from the browser, using the
//! access token obtained via Google Identity Services. No server in
//! between - requests go straight from the user's browser to
//! www.googleapis.com.

use gloo_net::http::Request;
use serde::{Deserialize, Serialize};

const API_BASE: &str = "https://www.googleapis.com/drive/v3";

/// Builds the files.list URL for one page of the appDataFolder listing.
/// `nextPageToken` must be explicitly requested in the `fields` mask (Drive's
/// partial-response API omits anything not listed there, including the
/// pagination token itself) or pagination silently breaks even though the
/// token would otherwise be present in the full response.
fn files_list_url(page_token: Option<&str>) -> String {
    match page_token {
        Some(token) => format!(
            "{API_BASE}/files?spaces=appDataFolder&fields=nextPageToken,files(id,name,modifiedTime,size)&pageSize=1000&pageToken={}",
            urlencoding::encode(token)
        ),
        None => format!(
            "{API_BASE}/files?spaces=appDataFolder&fields=nextPageToken,files(id,name,modifiedTime,size)&pageSize=1000"
        ),
    }
}

fn extract_next_page_token(body: &serde_json::Value) -> Option<String> {
    body.get("nextPageToken")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Builds the revisions.list URL for one page of a file's revision
/// history. Same nextPageToken-in-fields-mask requirement as
/// files_list_url - see that function's doc comment.
fn revisions_list_url(file_id: &str, page_token: Option<&str>) -> String {
    match page_token {
        Some(token) => format!(
            "{API_BASE}/files/{file_id}/revisions?fields=nextPageToken,revisions(id,modifiedTime,size)&pageSize=1000&pageToken={}",
            urlencoding::encode(token)
        ),
        None => format!(
            "{API_BASE}/files/{file_id}/revisions?fields=nextPageToken,revisions(id,modifiedTime,size)&pageSize=1000"
        ),
    }
}

fn revision_media_url(file_id: &str, revision_id: &str) -> String {
    format!("{API_BASE}/files/{file_id}/revisions/{revision_id}?alt=media")
}

fn revision_url(file_id: &str, revision_id: &str) -> String {
    format!("{API_BASE}/files/{file_id}/revisions/{revision_id}")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DriveFile {
    pub id: String,
    pub name: String,
    #[serde(default, rename = "modifiedTime")]
    pub modified_time: Option<String>,
    #[serde(default)]
    pub size: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DriveRevision {
    pub id: String,
    #[serde(default, rename = "modifiedTime")]
    pub modified_time: Option<String>,
    #[serde(default)]
    pub size: Option<String>,
}

async fn get_json(url: &str, token: &str) -> Result<serde_json::Value, String> {
    let resp = Request::get(url)
        .header("Authorization", &format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("network error: {e}"))?;

    let status = resp.status();
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("failed to parse response: {e}"))?;

    if !(200..300).contains(&status) {
        let msg = body
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("unknown error");
        return Err(format!("Drive API error ({status}): {msg}"));
    }
    Ok(body)
}

/// appDataFolder realistically holds a handful of files (this is a hidden
/// per-app storage bucket, not general Drive), so this cap exists purely
/// as a safety net against Google's own documented edge case where
/// `nextPageToken` can occasionally persist even when `files` comes back
/// empty (see https://issuetracker.google.com/issues/406305173) - without
/// a cap, a client hitting that bug would loop forever instead of
/// eventually surfacing an error. 50 pages * 1000 per page is far beyond
/// any realistic appDataFolder size.
const MAX_LIST_PAGES: usize = 50;

pub async fn list_appdata_files(token: &str) -> Result<Vec<DriveFile>, String> {
    let mut all_files = Vec::new();
    let mut page_token: Option<String> = None;

    for _ in 0..MAX_LIST_PAGES {
        let url = files_list_url(page_token.as_deref());
        let body = get_json(&url, token).await?;

        let files: Vec<DriveFile> =
            serde_json::from_value(body.get("files").cloned().unwrap_or(serde_json::json!([])))
                .map_err(|e| format!("failed to parse files: {e}"))?;
        all_files.extend(files);

        page_token = extract_next_page_token(&body);
        if page_token.is_none() {
            return Ok(all_files);
        }
    }

    Err(format!(
        "appDataFolder listing did not terminate after {MAX_LIST_PAGES} pages \
         ({} files collected so far) - this is unexpected and may indicate \
         a Drive API issue rather than a real amount of data.",
        all_files.len()
    ))
}

pub async fn list_revisions(token: &str, file_id: &str) -> Result<Vec<DriveRevision>, String> {
    // Note: even with full pagination, Google's own docs warn revisions.list
    // "might be incomplete for files with a large revision history... older
    // revisions might be omitted" - a caveat about Drive's server-side
    // behavior, not something a client can fully work around. Irrelevant in
    // practice for a small JSON backup file's handful of revisions, but not
    // something this fix can claim to guarantee away for every file type.
    let mut all_revisions = Vec::new();
    let mut page_token: Option<String> = None;

    for _ in 0..MAX_LIST_PAGES {
        let url = revisions_list_url(file_id, page_token.as_deref());
        let body = get_json(&url, token).await?;

        let revisions: Vec<DriveRevision> = serde_json::from_value(
            body.get("revisions")
                .cloned()
                .unwrap_or(serde_json::json!([])),
        )
        .map_err(|e| format!("failed to parse revisions: {e}"))?;
        all_revisions.extend(revisions);

        page_token = extract_next_page_token(&body);
        if page_token.is_none() {
            return Ok(all_revisions);
        }
    }

    Err(format!(
        "revisions listing for file {file_id} did not terminate after {MAX_LIST_PAGES} pages \
         ({} revisions collected so far) - this is unexpected and may indicate \
         a Drive API issue rather than a real amount of data.",
        all_revisions.len()
    ))
}

/// Downloads a specific revision's raw bytes.
pub async fn download_revision(
    token: &str,
    file_id: &str,
    revision_id: &str,
) -> Result<Vec<u8>, String> {
    let resp = Request::get(&revision_media_url(file_id, revision_id))
        .header("Authorization", &format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("network error: {e}"))?;

    if !resp.ok() {
        return Err(format!(
            "Drive API error downloading revision ({})",
            resp.status()
        ));
    }

    resp.binary()
        .await
        .map_err(|e| format!("failed to read revision bytes: {e}"))
}

pub async fn delete_revision(token: &str, file_id: &str, revision_id: &str) -> Result<(), String> {
    let resp = Request::delete(&revision_url(file_id, revision_id))
        .header("Authorization", &format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("network error: {e}"))?;

    if !resp.ok() {
        return Err(format!(
            "Drive API error deleting revision ({})",
            resp.status()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_list_url_without_token_targets_appdata_space() {
        let url = files_list_url(None);
        assert!(url.starts_with(API_BASE));
        assert!(url.contains("spaces=appDataFolder"));
        assert!(url.contains("fields=nextPageToken,files(id,name,modifiedTime,size)"));
        assert!(!url.contains("pageToken="));
    }

    #[test]
    fn files_list_url_requests_nextpagetoken_in_fields_mask() {
        // Drive's partial-response `fields` param omits anything not
        // listed, including nextPageToken itself - without this, pages
        // beyond the first would be silently unreachable even though the
        // API would otherwise have more results to give.
        let url = files_list_url(None);
        assert!(url.contains("fields=nextPageToken,"));
    }

    #[test]
    fn files_list_url_with_token_appends_encoded_page_token() {
        let url = files_list_url(Some("abc/def+ghi=="));
        assert!(url.contains("pageToken=abc%2Fdef%2Bghi%3D%3D"));
    }

    #[test]
    fn extract_next_page_token_present() {
        let body = serde_json::json!({"nextPageToken": "abc123", "files": []});
        assert_eq!(extract_next_page_token(&body), Some("abc123".to_string()));
    }

    #[test]
    fn extract_next_page_token_absent_means_last_page() {
        let body = serde_json::json!({"files": []});
        assert_eq!(extract_next_page_token(&body), None);
    }

    #[test]
    fn revisions_list_url_includes_file_id() {
        let url = revisions_list_url("FILE123", None);
        assert_eq!(
            url,
            format!(
                "{API_BASE}/files/FILE123/revisions?fields=nextPageToken,revisions(id,modifiedTime,size)&pageSize=1000"
            )
        );
    }

    #[test]
    fn revisions_list_url_requests_nextpagetoken_in_fields_mask() {
        let url = revisions_list_url("FILE123", None);
        assert!(url.contains("fields=nextPageToken,"));
    }

    #[test]
    fn revisions_list_url_with_token_appends_encoded_page_token() {
        let url = revisions_list_url("FILE123", Some("tok/en+="));
        assert!(url.contains("pageToken=tok%2Fen%2B%3D"));
    }

    #[test]
    fn revision_media_url_requests_media_alt() {
        let url = revision_media_url("FILE123", "REV456");
        assert_eq!(
            url,
            format!("{API_BASE}/files/FILE123/revisions/REV456?alt=media")
        );
    }

    #[test]
    fn revision_url_has_no_query_params() {
        let url = revision_url("FILE123", "REV456");
        assert_eq!(url, format!("{API_BASE}/files/FILE123/revisions/REV456"));
        assert!(!url.contains('?'));
    }

    #[test]
    fn drive_file_deserializes_with_all_fields() {
        let json = r#"{"id":"f1","name":"backup.json","modifiedTime":"2026-07-03T11:00:00Z","size":"866"}"#;
        let file: DriveFile = serde_json::from_str(json).unwrap();
        assert_eq!(file.id, "f1");
        assert_eq!(file.name, "backup.json");
        assert_eq!(file.modified_time.as_deref(), Some("2026-07-03T11:00:00Z"));
        assert_eq!(file.size.as_deref(), Some("866"));
    }

    #[test]
    fn drive_file_deserializes_with_missing_optional_fields() {
        let json = r#"{"id":"f1","name":"backup.json"}"#;
        let file: DriveFile = serde_json::from_str(json).unwrap();
        assert_eq!(file.modified_time, None);
        assert_eq!(file.size, None);
    }

    #[test]
    fn drive_revision_list_deserializes() {
        let json = r#"[
            {"id":"r1","modifiedTime":"2026-07-03T11:17:55.402Z","size":"866"},
            {"id":"r2","modifiedTime":"2026-07-03T11:18:03.918Z","size":"814"}
        ]"#;
        let revs: Vec<DriveRevision> = serde_json::from_str(json).unwrap();
        assert_eq!(revs.len(), 2);
        assert_eq!(revs[0].id, "r1");
        assert_eq!(revs[1].size.as_deref(), Some("814"));
    }
}
