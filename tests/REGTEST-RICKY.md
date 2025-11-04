# RICKY Rune Regtest Playbook

This document records the full workflow for minting the `RICKY` rune on regtest, transferring it to an Xverse wallet, minting wrapped RICKY on Spark, and exercising the backend API (gateway + Titan).

The steps assume you already have the regtest stack running (`USE_EXTERNAL_POSTGRES=1 ./infrastructure/scripts/run-everything-local.sh`).

---

## 1. Mint the RICKY rune once and send it to Xverse

### 1.1 Get the Xverse regtest addresses
- Open Xverse → Settings → Networks → Testnet Mode.
- Note the **Bitcoin taproot** address (starts with `bcrt1p…`). In these notes we use
  `bcrt1pctlwvnnt0873c3u8qu9sjzucj2a9sa0y2uyvk6h57klklg936dxs3850ml`.
- Note the **Spark regtest** address (starts with `sprt1…`) – used when minting wRicky later.
- Obtain the Xverse **compressed secp256k1 pubkey** (SatsConnect `getPublicKey` call). Example: `02abd1bbc56042816bcefb1fc608c1ebeea3268befecf54b241ca5f947962988d7`.

### 1.2 Mint the rune using the helper script
We added `tests/src/rune_manager.rs::mint_specific_rune` and a CLI wrapper `tests/src/bin/mint_ricky.rs`.

Run once (replace the address with your Xverse taproot address):

```bash
cargo run -p tests --bin mint_ricky -- bcrt1pctlwvnnt0873c3u8qu9sjzucj2a9sa0y2uyvk6h57klklg936dxs3850ml
```

The script:
- Faucets a fresh taproot key
- Etches the `RICKY` rune (hard-coded name)
- Mints the initial supply to the Xverse address
- Prints the rune id and the mint transaction id, e.g.
  ```
  Minted rune 'RICKY' with id 852:1 to bcrt1pctl…
  Mint transaction id: 7ea2f1304e49989280683e0575727f074442ed3fa73f34730a7296adef43a165
  ```

Keep the `rune_id` (`852:1`) and the mint `txid` for later.

### 1.3 Confirm the rune transaction on regtest
Inside the bitcoind container use `bitcoin-cli` (note we must use `-rpcconnect=bitcoind` because the daemon was started with `-rpcbind=bitcoind` and isn’t listening on `127.0.0.1`).

```bash
docker compose -f infrastructure/bitcoind.docker-compose.yml exec bitcoind \
  /opt/bitcoin-30.0/bin/bitcoin-cli -regtest \
  -rpcuser=bitcoin -rpcpassword=bitcoinpass -rpcconnect=bitcoind \
  getrawtransaction 7ea2f1304e49989280683e0575727f074442ed3fa73f34730a7296adef43a165 true
```

You should see:
- An `OP_RETURN` runestone output (`6a5d…`) representing the etch/mint.
- Output `n=1` to the Xverse taproot address.

You can also check the height / overall node status:

```bash
docker compose -f infrastructure/bitcoind.docker-compose.yml exec bitcoind \
  /opt/bitcoin-30.0/bin/bitcoin-cli -regtest \
  -rpcuser=bitcoin -rpcpassword=bitcoinpass -rpcconnect=bitcoind getblockchaininfo
```

---

## 2. Watch Titan indexing progress
Titan may take a couple of minutes to ingest the block. Until it finishes the APIs below return 404.

- Raw root endpoint (always works):
  ```bash
  curl -i http://localhost:3030/
  # HTTP/1.1 200 OK  {"status":"ok"}
  ```
- Rune metadata (works after indexing):
  ```bash
  curl -i http://localhost:3030/v1/runes/852:1
  # Expect HTTP/1.1 200 with JSON once indexed
  ```
- Address view (also 404 until indexed):
  ```bash
  curl -i http://localhost:3030/v1/address/bcrt1pctlwvnnt0873c3u8qu9sjzucj2a9sa0y2uyvk6h57klklg936dxs3850ml
  ```

Watch `docker compose -f infrastructure/bitcoind.docker-compose.yml logs -f titan` for messages such as `Synced to tip 879`.

If Titan never indexes the rune, consider swapping to the Maestro API instead (see mainnet discussion).

---

## 3. Bridging RICKY → wRicky on Spark
Once Titan exposes the rune data, run the bridge flow.

1. Request a deposit address bound to the Xverse pubkey:
   ```bash
   curl -sS -X POST http://localhost:8060/api/user/get-btc-deposit-address \
     -H 'content-type: application/json' \
     -d '{"user_public_key":"02abd1bbc56042816bcefb1fc608c1ebeea3268befecf54b241ca5f947962988d7","rune_id":"852:1","amount":100000}'
   # -> {"address":"bcrt1p484…"}
   ```

2. From Xverse, send the desired amount of RICKY to that address. Wait for 6 confirmations. Note the new `txid`/`vout` in Xverse (or via `bitcoin-cli`).

3. Submit the Bridge call:
   ```bash
   curl -sS -X POST http://localhost:8060/api/user/bridge-runes \
     -H 'content-type: application/json' \
     -d '{"btc_address":"bcrt1p484lu0aq5xzgpes25kekl6h9c4u0ut6w88y82qmjtrhq63e7xanqtknk63","bridge_address":"sprt1pgssymuyndyt3hley5rn0x3jxddmtlmx7wvvffp82kw7x7sezsvt7fknkfycpk","txid":"<bridge_txid>","vout":<bridge_vout>}'
   ```

4. Within ~10 seconds the gateway mints wRicky to the Spark address. Open Xverse (Spark → Regtest) to see the BLTKN balance increase. Logs in `gateway.log` / `titan.log` confirm the mint.

5. There’s no need to exit back to Bitcoin for this sandbox testing, but the reverse endpoint is `POST /api/user/exit-spark` when you’re ready.

---

## 4. Other helper commands
- Review the mint transaction in Titan once indexed:
  ```bash
  curl -s http://localhost:3030/v1/transaction/7ea2f1304e49989280683e0575727f074442ed3fa73f34730a7296adef43a165 | jq
  ```
- Scan the UTXO set inside the container:
  ```bash
  docker compose -f infrastructure/bitcoind.docker-compose.yml exec bitcoind \
    /opt/bitcoin-30.0/bin/bitcoin-cli -regtest \
    -rpcuser=bitcoin -rpcpassword=bitcoinpass -rpcconnect=bitcoind \
    scantxoutset start '[{"scanobjects":["addr(bcrt1pctlwvnnt0873c3u8qu9sjzucj2a9sa0y2uyvk6h57klklg936dxs3850ml)"]}]'
  ```

---

## 5. Summary of code changes
- `tests/src/rune_manager.rs`: added `mint_specific_rune` and exported `unite_unspent_utxos` for reuse.
- `tests/src/bin/mint_ricky.rs`: new binary to create the RICKY rune and send it to any taproot address.

Those changes let us mint a deterministic rune for front-end testing and feed Xverse with real regtest assets.
