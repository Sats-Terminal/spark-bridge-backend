# Runes ↔ Spark Backend Integration Guide

This document explains how to integrate with the gateway backend to bridge Runes between Bitcoin and Spark, and how to initiate Spark exits back to Bitcoin. It also covers the available API endpoints, request/response formats, and operational considerations.

Word marks such as Spark™ and Rune™ belong to their respective owners.

---

## Overview

The bridge consists of the following services:

- **Gateway** (`gateway_main`): primary HTTP API exposed to clients.
- **Verifiers** (`verifier_main` ×3): independently validate deposits and participate in FROST signing.
- **BTC Indexer** (`btc_indexer_main`): monitors Bitcoin transactions and provides confirmations to verifiers.
- **Spark Balance Checker** (`spark_balance_checker_main`): auxiliary Spark service.

All user-facing functionality is provided by the gateway service. This guide assumes the stack is running locally via:

```bash
USE_EXTERNAL_POSTGRES=1 ./infrastructure/scripts/run-everything-local.sh -f
```

Configuration from `.env.mainnet` is automatically loaded, providing Maestro credentials for Rune metadata and transaction queries.

The gateway listens on `http://localhost:8060` by default; replace the hostname/port if you deploy elsewhere. All endpoints accept and return JSON.

---

## Conventions and Shared Concepts

### Rune Amounts and Divisibility

- Request payloads use **human-readable rune units** (i.e., already accounting for divisibility).  
- The gateway fetches metadata from Maestro to convert human units into base units when running on mainnet.  
- Example: `amount = 500000000` with divisibility `2` becomes `50,000,000,000` base units internally.

### Bitcoin Deposits

- Deposits must reach **6 confirmations** before verifiers approve a bridge transaction (`BTC_BLOCK_CONFIRMATION_HEIGHT` constant).  
- All Bitcoin addresses returned by the gateway are tweaked Taproot addresses bound to the requesting Musig ID and nonce.
- Outpoints (`txid` + zero-based `vout`) must be recorded off-chain and supplied back to the gateway.

### Spark Deposits

- Spark exits require a **pre-signed Taproot input** that spends the off-chain Spark paying transaction.  
- Signatures must be Schnorr signatures with sighash `ALL|ANYONECANPAY`.

### Error Handling

Errors are returned as plain strings with HTTP status:

- `400 Bad Request`: invalid payloads or malformed addresses/keys.  
- `500 Internal Server Error`: downstream failures (FROST, Spark, Maestro, database).

The stack logs detailed context to `logs/*.log`.

---

## Bridging Bitcoin Runes to Spark wRunes

1. **Request a deposit address**

   ```http
   POST /api/user/get-btc-deposit-address
   Content-Type: application/json

   {
     "user_public_key": "02af6d71243386e3a24a23ebf47ea91dcf0d114b8ec29163ed9716e9b14a8fe3d8",
     "rune_id": "840002:1",
     "amount": 500000000
   }
   ```

   Response:

   ```json
   { "address": "bc1p25vrwa7qgvjwj93mduhvjt5g9kz398l5dk759t7tyvgtd4uas2ts22dakr" }
   ```

   - `user_public_key`: 33-byte compressed Secp256k1 public key (hex).  
   - `rune_id`: height/tx-index string (`"<block_height>:<tx_index>"`).  
   - `amount`: human-readable rune units.

2. **Send the deposit**

   - Transfer the requested rune amount to the returned Taproot address.  
   - Wait for at least **6 confirmations**.  
   - Record `txid` and `vout` for the output carrying the rune payment (verify with Maestro or your indexer).

3. **Trigger the bridge**

   ```http
   POST /api/user/bridge-runes
   Content-Type: application/json

   {
     "btc_address": "bc1p25vrwa7qgvjwj93mduhvjt5g9kz398l5dk759t7tyvgtd4uas2ts22dakr",
     "bridge_address": "sp1pgssymuyndyt3hley5rn0x3jxddmtlmx7wvvffp82kw7x7sezsvt7fknzlq8gf",
     "txid": "58b16053e0865ce52c41b4d04f91725db0e764d33fe533c22b513b3aaab088ef",
     "vout": 1
   }
   ```

   - `btc_address`: deposit address obtained in step 1.  
   - `bridge_address`: Spark address (must match the active Spark network).  
   - `txid`: Bitcoin transaction hash (hex, lowercase/uppercase accepted).  
   - `vout`: zero-based index of the rune-carrying output.

   Response: `{}` (minting proceeds asynchronously).

4. **Observe the mint**

   - Verifiers track the outpoint via BTC indexer callbacks and approve once six confirmations are observed.  
   - The gateway then sends a Spark mint transaction using FROST-signed MuSig keys.  
   - Monitor `logs/gateway.log`, the Spark wallet, or the output of `/api/metadata/wrunes`.

---

## Bridging Spark wRunes Back to Bitcoin

1. **Request a Spark deposit address**

   ```http
   POST /api/user/get-spark-deposit-address
   Content-Type: application/json

   {
     "user_public_key": "02af6d71243386e3a24a23ebf47ea91dcf0d114b8ec29163ed9716e9b14a8fe3d8",
     "rune_id": "840002:1",
     "amount": 500000000
   }
   ```

   Response:

   ```json
   { "address": "sprt1p..." }
   ```

2. **Send the wRunes to the provided Spark address.**

   Capture the resulting paying input:

   - Spark transaction ID / VOUT providing the payment.  
   - Satoshis reserved for Bitcoin miner fees (`sats_amount`).  
   - Bitcoin exit address (`btc_exit_address`).  
   - `NONE|ANYONECANPAY` Taproot signature (`none_anyone_can_pay_signature`) covering the paying input.

3. **Submit the exit request**

   ```http
   POST /api/user/exit-spark
   Content-Type: application/json

   {
     "spark_address": "sp1pgssymuyndyt3hley5rn0x3jxddmtlmx7wvvffp82kw7x7sezsvt7fknzlq8gf",
     "paying_input": {
       "txid": "ef1cfd3cbdd37aa7189d9d0d836f1519a6364c65014d4cba1e0fb7cd43839c5d",
       "vout": 1,
       "btc_exit_address": "bc1penmn2qtxu5dfal5aycgjj4ul83g7dv649x59e4fgvfncma7nfkjqxzkgnh",
       "sats_amount": 5000,
       "none_anyone_can_pay_signature": "a9c4f3...7f1b"
     }
   }
   ```

   Response: `{}` (spend is executed asynchronously).  
   Verifiers confirm the Spark deposit and co-sign a Bitcoin transaction delivering the rune-equivalent UTXO to `btc_exit_address`.

---

## Metadata and Discovery

- `GET /api/metadata/wrunes` — returns all cached wRune metadata entries. Useful for discovering tickers, decimals, issuer musig public key, and Spark token identifiers.  
- Each entry mirrors the format:

  ```json
  {
    "rune_id": "840002:1",
    "rune_metadata": { ... },
    "wrune_metadata": {
      "token_identifier": "a78995eca11b4190fe735f7b6e173c07ac1ce24b71ee93360f10a44bb99a6adb",
      "token_ticker": "9ATcvZ",
      "token_name": "PEPEISBITCOIN",
      "decimals": 2,
      "max_supply": 42069696969696900,
      "icon_url": "https://icon.unisat.io/icon/runes/PEPE%E2%80%A2IS%E2%80%A2BITCOIN",
      "original_rune_id": "840002:1"
    },
    "issuer_public_key": "032bad18f07f17a4e4e569b0ca15365ce6e1e3bfddf73ed0e6dc24b95db6ad1a21",
    "bitcoin_network": "bitcoin",
    "spark_network": "Mainnet",
    "created_at": "2025-11-05T19:07:21.261068Z",
    "updated_at": "2025-11-05T19:07:21.321804Z"
  }
  ```
- `GET /api/user/activity/{user_public_key}` — returns the list of Rune→Spark bridge attempts initiated by the given compressed Secp256k1 public key. Each item contains the requested amount, deposit address, target Spark address, overall lifecycle status (`address_issued`, `waiting_for_confirmations`, `ready_for_mint`, `minted`, `spent`, or `failed`), cached wRune metadata, and (when available) the latest confirmation count retrieved from Maestro for the associated Bitcoin transaction. Example:

  ```json
  [
    {
      "rune_id": "840002:1",
      "amount": 500000000,
      "btc_deposit_address": "bc1p25vrwa7qgvjwj93mduhvjt5g9kz398l5dk759t7tyvgtd4uas2ts22dakr",
      "spark_bridge_address": "sp1pgssymuyndyt3hley5rn0x3jxddmtlmx7wvvffp82kw7x7sezsvt7fknzlq8gf",
      "status": "minted",
      "confirmations": 9,
      "txid": "58b16053e0865ce52c41b4d04f91725db0e764d33fe533c22b513b3aaab088ef",
      "vout": 1,
      "wrune_metadata": {
        "token_identifier": "a78995eca11b4190fe735f7b6e173c07ac1ce24b71ee93360f10a44bb99a6adb",
        "token_ticker": "9ATcvZ",
        "token_name": "PEPEISBITCOIN",
        "decimals": 2,
        "max_supply": 42069696969696900,
        "icon_url": "https://icon.unisat.io/icon/runes/PEPE%E2%80%A2IS%E2%80%A2BITCOIN",
        "original_rune_id": "840002:1"
      }
    }
  ]
  ```
- `GET /api/bridge/transaction/{txid}` — returns a single bridge summary for the specified Bitcoin transaction hash. The payload mirrors the structure of a single entry from the user activity list but is convenient when the txid is known in advance (for example, after submitting a bridge request). Example:

  ```json
  {
    "rune_id": "840002:1",
    "amount": 500000000,
    "btc_deposit_address": "bc1p25vrwa7qgvjwj93mduhvjt5g9kz398l5dk759t7tyvgtd4uas2ts22dakr",
    "spark_bridge_address": "sp1pgssymuyndyt3hley5rn0x3jxddmtlmx7wvvffp82kw7x7sezsvt7fknzlq8gf",
    "status": "minted",
    "confirmations": 9,
    "txid": "58b16053e0865ce52c41b4d04f91725db0e764d33fe533c22b513b3aaab088ef",
    "vout": 1,
    "wrune_metadata": {
      "token_identifier": "a78995eca11b4190fe735f7b6e173c07ac1ce24b71ee93360f10a44bb99a6adb",
      "token_ticker": "9ATcvZ",
      "token_name": "PEPEISBITCOIN",
      "decimals": 2,
      "max_supply": 42069696969696900,
      "icon_url": "https://icon.unisat.io/icon/runes/PEPE%E2%80%A2IS%E2%80%A2BITCOIN",
      "original_rune_id": "840002:1"
    }
  }
  ```

- `DELETE /api/user/bridge-request/{btc_address}` — cancels a pending bridge request that has not yet received a Bitcoin deposit. The path parameter is the exact Taproot deposit address previously issued by `get-btc-deposit-address`. The gateway only deletes the request if **no** UTXO has been recorded for that address; otherwise it returns HTTP 400.

---

## Healthcheck

- `POST /health` — readiness probe. Returns HTTP 200 with `{}` when the gateway, verifiers, and storage are reachable.

---

## Internal Verifier Endpoint (Operational Only)

- `POST /api/verifier/notify-runes-deposit`

  ```json
  {
    "verifier_id": 2,
    "out_point": { "txid": "...", "vout": 1 },
    "sats_fee_amount": 546,
    "status": { "confirmed": {} }
  }
  ```

  Used by verifier services after BTC indexer callbacks to inform the gateway about deposit status changes. Do not expose this endpoint to untrusted clients.

---

## Operational Tips

- Maestro metadata is required for divisibility; missing credentials will cause the gateway to treat amounts as already normalized, leading to mismatches.
- Rerunning a bridge for the same `rune_id` reuses the existing wRune token (same Spark ticker and identifier).
- When testing on mainnet, expect ~1 hour for six confirmations before a Spark mint executes.
- Logs are stored under `logs/` (`gateway.log`, `verifier_*.log`, `btc_indexer.log`). Tail them for real-time status.
- Database cleanup commands (optional):

  ```sql
  -- Remove stale issuer musig entries for a rune_id
  DELETE FROM gateway.sign_session
    WHERE public_key = '<hex>' AND rune_id = '840002:1';

  DELETE FROM gateway.musig_identifier
    WHERE public_key = '<hex>' AND rune_id = '840002:1' AND is_issuer = true;
  ```

---

## Example Client Flow (Pseudo-code)

```text
1. POST /api/user/get-btc-deposit-address → returns bc1p…
2. Broadcast Bitcoin rune transfer to bc1p…, wait for 6 confirmations.
3. POST /api/user/bridge-runes with txid/vout → returns {}.
4. Poll Spark wallet or /api/metadata/wrunes for minted balance.
5. To exit, POST /api/user/get-spark-deposit-address → sprt1…
6. Send wRunes to sprt1…, prepare paying input signature.
7. POST /api/user/exit-spark with paying_input → returns {}.
8. Observe Bitcoin wallet for inbound settlement transaction.
```

---

## Support

File issues or questions in the repository, and inspect the service logs for troubleshooting details. Ensure the verifiers and btc_indexer are running and reachable whenever you integrate with the gateway.
