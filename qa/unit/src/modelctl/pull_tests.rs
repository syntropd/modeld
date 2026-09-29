//! Unit QA tests for modelctl pull model resolution.

use modelctl::cmd::pull::resolve::{resolve_model, select_gguf_file};
use modelctl::reqwest::Client;

#[tokio::test]
async fn test_resolve_curated_aliases_offline() {
    let client = Client::new();

    let m1 = resolve_model(&client, "qwen2.5:0.5b", None, None).await.unwrap();
    assert_eq!(m1.name, "qwen2.5");
    assert_eq!(m1.tag, "0.5b");
    assert!(m1.download_url.contains("Qwen/Qwen2.5-0.5B-Instruct-GGUF"));

    let m2 = resolve_model(&client, "llama3.2:1b", None, Some("custom:v1")).await.unwrap();
    assert_eq!(m2.name, "custom");
    assert_eq!(m2.tag, "v1");

    let m3 = resolve_model(&client, "smollm2:360m", None, Some("fast")).await.unwrap();
    assert_eq!(m3.name, "smollm2");
    assert_eq!(m3.tag, "fast");

    let err = resolve_model(&client, "unknown-model", None, None).await.unwrap_err();
    assert!(err.to_string().contains("Unknown model alias"));
}

#[test]
fn test_select_gguf_file_preference_and_empty() {
    assert!(select_gguf_file(&[], None).is_err());

    let files = vec![
        "model-q8_0.gguf".to_string(),
        "model-q4_k_m.gguf".to_string(),
        "model-f16.gguf".to_string(),
    ];
    assert_eq!(select_gguf_file(&files, None).unwrap(), "model-q4_k_m.gguf");
    assert_eq!(select_gguf_file(&files, Some("q8_0")).unwrap(), "model-q8_0.gguf");
    assert!(select_gguf_file(&files, Some("nonexistent")).is_err());
}
