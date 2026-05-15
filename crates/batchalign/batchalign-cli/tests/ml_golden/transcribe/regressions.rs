use crate::ml_golden::regression_fixtures::harness::run_fixture;

#[tokio::test]
async fn transcribe_regression_001() {
    run_fixture("transcribe", "transcribe-regression-001").await
}

#[tokio::test]
async fn transcribe_regression_002() {
    run_fixture("transcribe", "transcribe-regression-002").await
}

#[tokio::test]
async fn transcribe_regression_003() {
    run_fixture("transcribe", "transcribe-regression-003").await
}

#[tokio::test]
async fn transcribe_regression_004() {
    run_fixture("transcribe", "transcribe-regression-004").await
}

#[tokio::test]
async fn transcribe_regression_005() {
    run_fixture("transcribe", "transcribe-regression-005").await
}

#[tokio::test]
async fn transcribe_regression_006() {
    run_fixture("transcribe", "transcribe-regression-006").await
}
