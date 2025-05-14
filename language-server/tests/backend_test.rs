use tokio::time::{Duration, sleep};

#[tokio::test]
async fn test_basic() {
    sleep(Duration::from_secs(2)).await;
    assert_eq!(1 + 1, 2);
}
