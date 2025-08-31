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

    flow_processor_task.await.unwrap();
}
