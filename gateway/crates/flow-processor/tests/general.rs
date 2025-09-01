use gateway_flow_processor::create_flow_processor;
use gateway_flow_processor::flow_sender::TypedMessageSender;
use gateway_flow_processor::types::TestingRequest;
use global_utils::logger::init_logger;
use persistent_storage::{config::PostgresDbCredentials, init::PostgresRepo};
use tokio;

#[tokio::test]
async fn test_1() {
    let _guard = init_logger();
    //todo: fix tests & make more flexible
    tracing::info!("Testing flow processor");

    let url = "postgresql://postgres:postgres@localhost:5433/postgres".to_string();
    let storage = PostgresRepo::from_config(PostgresDbCredentials { url }).await.unwrap();
    let (mut flow_processor, flow_sender) = create_flow_processor(storage);

    let flow_processor_task = tokio::task::spawn(async move {
        flow_processor.run().await;
    });

    let testing_request = TestingRequest {
        message: "test".to_string(),
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
    //todo: fix tests & make more flexible
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

#[tokio::test]
async fn test_3() {
    let _guard = init_logger();
    //todo: fix tests & make more flexible
    tracing::info!("Testing flow processor");

    let url = "postgresql://postgres:postgres@localhost:5433/postgres".to_string();
    let storage = PostgresRepo::from_config(PostgresDbCredentials { url }).await.unwrap();
    let (mut flow_processor, flow_sender) = create_flow_processor(storage);

    let flow_processor_task = tokio::task::spawn(async move {
        flow_processor.run().await;
    });

    let flow_sender_1 = flow_sender.clone();
    let flow_sender_2 = flow_sender.clone();
    let flow_sender_3 = flow_sender.clone();

    let testing_request_1 = TestingRequest {
        message: "test_1".to_string(),
        n_seconds: 1,
        n_runs: 10,
    };

    let testing_request_2 = TestingRequest {
        message: "test_2".to_string(),
        n_seconds: 2,
        n_runs: 5,
    };

    let testing_request_3 = TestingRequest {
        message: "test_3".to_string(),
        n_seconds: 3,
        n_runs: 3,
    };

    let task_1 = tokio::spawn(async move {
        let testing_response_1 = flow_sender_1.send(testing_request_1).await.unwrap();
        println!("Testing response 1: {:?}", testing_response_1.message);
    });
    let task_2 = tokio::spawn(async move {
        let testing_response_2 = flow_sender_2.send(testing_request_2).await.unwrap();
        println!("Testing response 2: {:?}", testing_response_2.message);
    });
    let task_3 = tokio::spawn(async move {
        let testing_response_3 = flow_sender_3.send(testing_request_3).await.unwrap();
        println!("Testing response 3: {:?}", testing_response_3.message);
    });

    tokio::time::sleep(tokio::time::Duration::from_secs(8)).await;

    flow_sender.shutdown().await;

    for task in [task_1, task_2, task_3] {
        task.await.unwrap();
    }

    flow_processor_task.await.unwrap();
}
