use tokio;
use gateway_flow_processor::create_flow_processor;
use persistent_storage::{init::PostgresRepo, config::PostgresDbCredentials};
use gateway_flow_processor::types::TestingRequest;
use gateway_flow_processor::flow_sender::TypedMessageSender;
use global_utils::logger::init_logger;

#[tokio::test]
async fn test_1() {
    let _guard = init_logger();

    tracing::info!("Testing flow processor");

    let url = "postgresql://postgres:postgres@localhost:5433/postgres".to_string();
    let storage = PostgresRepo::from_config(PostgresDbCredentials { url }).await.unwrap();
    let (mut flow_processor, flow_sender) = create_flow_processor(storage);

    let flow_processor_task = tokio::task::spawn(async move {
        flow_processor.run().await;
    });

    let testing_request = TestingRequest {
        thread_name: "test".to_string(),
        n_seconds: 1,
        n_runs: 1,
    };

    let testing_response = flow_sender.send(testing_request).await;

    match testing_response {
        Ok(response) => {
            println!("Testing response: {:?}", response.message);
        }
        Err(e) => {
            println!("Error response: {:?}", e);
        }
    }

    flow_sender.shutdown().await;

    flow_processor_task.await.unwrap();
}


#[tokio::test]
async fn test_2() {
    let _guard = init_logger();

    let (tx1, mut rx1) = tokio::sync::mpsc::channel::<String>(10);
    let (tx2, mut rx2) = tokio::sync::mpsc::channel::<String>(10);

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