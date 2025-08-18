import {
  TitanEventType,
  TitanHttpClient,
  TitanTcpClient,
} from "@titanbtcio/sdk";
import logger from "../src/logger";

// taken from:

const BASE_URL = "http://localhost:3030/";

function testTcpSubscription() {
  // Create a TCP client instance with auto-reconnect enabled.
  const tcpClient = new TitanTcpClient("localhost", 3030, {
    autoReconnect: true, // Automatically reconnect if disconnected
    reconnectDelayMs: 5000, // Wait 5 seconds between reconnection attempts
  });

  // Listen for incoming events.
  tcpClient.on("event", (event) => {
    console.log("[TcpListener] Received event:", event);
  });

  // Listen for errors.
  tcpClient.on("error", (err) => {
    console.error("[TcpListener] TCP Client Error:", err);
  });

  // Listen for when the connection closes.
  tcpClient.on("close", () => {
    console.log("TCP connection closed.");
  });

  // Listen for reconnection notifications.
  tcpClient.on("reconnect", () => {
    console.log("TCP client reconnected.");
  });

  // Start the subscription.
  tcpClient.subscribe({
    subscribe: [
      TitanEventType.RuneEtched,
      TitanEventType.RuneMinted,
      TitanEventType.NewBlock,
      TitanEventType.RuneBurned,
      TitanEventType.Reorg,
      TitanEventType.TransactionsReplaced,
      TitanEventType.TransactionsAdded,
      TitanEventType.RuneTransferred,
      TitanEventType.AddressModified,
    ],
  });
}

async function testApi() {
  const httpClient = new TitanHttpClient(BASE_URL);
  try {
    console.log("runes");
    let mempool_entries = await httpClient.getAllMempoolEntries();
    console.log("status1");
    let status = await httpClient.getStatus();
    console.log("status2");
    let runes = await httpClient.getRunes();

    console.log(`Runes response: ${JSON.stringify(runes)}, 
            Http api status: ${JSON.stringify(status)}, 
            Mempool entries: ${JSON.stringify(mempool_entries)}`);
  } catch (error) {
    logger.error(`API call failed: ${error}`);
  }
}

logger.info("before test api");
await testApi();
logger.info("after test api");
testTcpSubscription();
