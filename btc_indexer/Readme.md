## Btc verifier

Uses Maestro (mainnet/testnet) or Titan (local regtest) indexers for verifying transactions in the Bitcoin network. Set `BITCOIN_NETWORK` to `regtest` to keep using the bundled Titan service, otherwise provide `MAESTRO_API_URL` and `MAESTRO_API_KEY` and the indexer will switch to Maestro automatically.

### Docker draft deployment

[Docker setup][1]

### How to run?

```bash
  cargo run --features swagger
```

[1]: https://github.com/SaturnBTC/Titan/blob/master/DOCKER_SETUP.md
