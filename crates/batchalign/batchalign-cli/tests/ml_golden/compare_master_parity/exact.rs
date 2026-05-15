use super::harness::run_compare_master_parity;

#[tokio::test]
async fn parity_compare_eng_exact() {
    run_compare_master_parity("eng_compare_exact").await;
}

#[tokio::test]
async fn parity_compare_eng_multi_exact() {
    run_compare_master_parity("eng_compare_multi_exact").await;
}
