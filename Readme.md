# Runes <-> Spark bridge
Project idea is described in docs which you can find [here][1] or in this [folder](./docs).

[1]: https://docs.google.com/document/d/120hl_wwvCOdpYgpEi_H5Lwt-jwIGA9miWjgOPo5QS-s/edit?tab=t.0

## Services

### Spark Balance Checker

The service is responsible for querying spark nodes in order to get the balance for the specified **spark_address** and **rune_id**.

#### How to use

1. Fill the ***spark_balance_checker/config.rs***
2. Run the following command: `cargo run --bin spark-balance-checker-main`