import logger from "../src/logger";
import Client from "bitcoin-core";

const client = new Client({
  password: "bitcoinpass",
  username: "bitcoin",
  host: "http://127.0.0.1",
  port: 18443,
  network: "regtest",
});

async function main() {
  try {
    const info = await client.getBlockchainInfo();
    logger.info(info);
  } catch (error) {
    logger.error(error);
  }
}

main();
