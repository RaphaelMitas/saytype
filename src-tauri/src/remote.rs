use serde::Deserialize;

#[derive(Deserialize)]
struct HealthResponse {
    status: String,
}

#[derive(Deserialize)]
struct TranscribeResponse {
    success: bool,
    text: Option<String>,
    error: Option<String>,
}

/// Check if the remote transcription server is ready.
pub async fn check_health(server_url: &str) -> Result<bool, String> {
    let url = format!("{}/health", server_url.trim_end_matches('/'));
    let resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("Failed to connect to server: {}", e))?;

    let health: HealthResponse = resp
        .json()
        .await
        .map_err(|e| format!("Invalid health response: {}", e))?;

    Ok(health.status == "ready")
}

/// Send audio to the remote server for transcription.
pub async fn transcribe(server_url: &str, audio_path: &str) -> Result<String, String> {
    let url = format!("{}/transcribe", server_url.trim_end_matches('/'));
    let audio_bytes = tokio::fs::read(audio_path)
        .await
        .map_err(|e| format!("Failed to read audio file: {}", e))?;

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .header("Content-Type", "application/octet-stream")
        .body(audio_bytes)
        .send()
        .await
        .map_err(|e| format!("Failed to send audio to server: {}", e))?;

    let result: TranscribeResponse = resp
        .json()
        .await
        .map_err(|e| format!("Invalid transcription response: {}", e))?;

    if result.success {
        result.text.ok_or_else(|| "No text in response".to_string())
    } else {
        Err(result.error.unwrap_or_else(|| "Unknown error".to_string()))
    }
}
