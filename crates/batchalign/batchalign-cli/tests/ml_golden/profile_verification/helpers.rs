use serde_json::Value;

pub const ENG_TEXT_FIXTURE: &str = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant
@ID:\teng|test|PAR|||||Participant|||
*PAR:\tthe dog is running .
*PAR:\tI like cats .
*PAR:\tshe went to the store .
@End
";

/// Query the `/health` endpoint and return the parsed JSON value.
///
/// Uses `serde_json::Value` for flexible field access without coupling
/// to the exact `HealthResponse` struct layout — profile key formats
/// may evolve and this keeps assertions readable.
pub async fn query_health(client: &reqwest::Client, base_url: &str) -> Value {
    let resp = client
        .get(format!("{base_url}/health"))
        .send()
        .await
        .expect("health request failed");
    assert_eq!(resp.status(), 200);
    resp.json::<Value>().await.expect("health parse failed")
}

/// Extract `live_worker_keys` from a health JSON response as a `Vec<String>`.
pub fn extract_worker_keys(health: &Value) -> Vec<String> {
    health["live_worker_keys"]
        .as_array()
        .expect("live_worker_keys should be an array")
        .iter()
        .map(|v| {
            v.as_str()
                .expect("worker key should be a string")
                .to_owned()
        })
        .collect()
}
