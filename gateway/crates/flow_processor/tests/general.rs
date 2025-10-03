use global_utils::logger::init_logger;
use tokio;

const BUFFER: usize = 10;

#[tokio::test]
async fn test_select_channel() {
    let _guard = init_logger();

    let (tx1, mut rx1) = tokio::sync::mpsc::channel::<String>(BUFFER);
    let (tx2, mut rx2) = tokio::sync::mpsc::channel::<String>(BUFFER);

    let task = tokio::spawn(async move {
        loop {
            tokio::select! {
                msg1 = rx1.recv() => {
                    tracing::info!("rx1 received: {:?}", msg1);
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
                msg2 = rx2.recv() => {
                    tracing::info!("rx2 received: {:?}", msg2);
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
            tracing::info!("loop");
        }
    });

    tx1.send("1".to_string()).await.unwrap();
    tx2.send("2".to_string()).await.unwrap();

    let _ = task.await.unwrap();
}
