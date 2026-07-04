//! Calls to the Google Drive REST API directly from the browser, using the
//! access token obtained via Google Identity Services. No server in
//! between - requests go straight from the user's browser to
//! www.googleapis.com.

use gloo_net::http::Request;
use serde::{Deserialize, Serialize};

const API_BASE: &str = "https://www.googleapis.com/drive/v3";

fn files_list_url() -> String {
    format!(
        "{API_BASE}/files?spaces=appDataFolder&fields=files(id,name,modifiedTime,size)&pageSize=100"
    )
}

fn revisions_list_url(file_id: &str) -> String {
    format!("{API_BASE}/files/{file_id}/revisions?fields=revisions(id,modifiedTime,size)")
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

pub async fn list_appdata_files(token: &str) -> Result<Vec<DriveFile>, String> {
    let body = get_json(&files_list_url(), token).await?;
    let files: Vec<DriveFile> =
        serde_json::from_value(body.get("files").cloned().unwrap_or(serde_json::json!([])))
            .map_err(|e| format!("failed to parse files: {e}"))?;
    Ok(files)
}

pub async fn list_revisions(token: &str, file_id: &str) -> Result<Vec<DriveRevision>, String> {
    let body = get_json(&revisions_list_url(file_id), token).await?;
    let revisions: Vec<DriveRevision> = serde_json::from_value(
        body.get("revisions")
            .cloned()
            .unwrap_or(serde_json::json!([])),
    )
    .map_err(|e| format!("failed to parse revisions: {e}"))?;
    Ok(revisions)
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
    fn files_list_url_targets_appdata_space() {
        let url = files_list_url();
        assert!(url.starts_with(API_BASE));
        assert!(url.contains("spaces=appDataFolder"));
        assert!(url.contains("fields=files(id,name,modifiedTime,size)"));
    }

    #[test]
    fn revisions_list_url_includes_file_id() {
        let url = revisions_list_url("FILE123");
        assert_eq!(
            url,
            format!("{API_BASE}/files/FILE123/revisions?fields=revisions(id,modifiedTime,size)")
        );
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
