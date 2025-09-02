This folder contains files that is used as helpers for transaction creatin in bitcoin core.

make your code prettier: `npx prettier --write '**/*.ts'`

Useful commands for execution in bitcoin-cli:

look for info about balance on address
```bash
bitcoin-cli -rpcconnect=127.0.0.1 -rpcport=18443 -rpcpassword=bitcoinpass -rpcuser=bitcoin scantxoutset start '[{"desc":"addr(insert_you_address)"}]'
```


mine blocks on address to receive rewards:
```bash
bitcoin-cli -rpcconnect=127.0.0.1 -rpcport=18443 -rpcpassword=bitcoinpass -rpcuser=bitcoin generatetoaddress some_number_of_blocks insert_you_address
```


to mine blocks and retreive an utxo: 
```bash
bitcoin-cli -rpcconnect=127.0.0.1 -rpcport=18443 -rpcpassword=bitcoinpass -rpcuser=bitcoin generatetoaddress 1 bcrt1pc4v4fh5cy3pcrqxvye0m20gzfgxskn5p5qpmhskgym3md8m3cx5qn59c5q
#[
#  "5c2949d363b3da4265ce6c85facae134e81e0dab783b83bb7c761dba4c2db9f2"
#]

bitcoin-cli -rpcconnect=127.0.0.1 -rpcport=18443 -rpcpassword=bitcoinpass -rpcuser=bitcoin generatetoaddress 100 bcrt1pc4v4fh5cy3pcrqxvye0m20gzfgxskn5p5qpmhskgym3md8m3cx5qn59c5q
#[
#  "798d4206f1547970829c1484b380753c03d5a79c7703ae5601fe38e6a2100dd2",
#  "38c2d89621738ac90e57b57f73c734c1cfcff374911e734c3a7158ce3f48bb4d",
#  "4a45ef08c36dff73766a42c766c42a260f620d806990bf0b099398b06ee0723f",
#  ....
#]

 bitcoin-cli -rpcconnect=127.0.0.1 -rpcport=18443 -rpcpassword=bitcoinpass -rpcuser=bitcoin getblock 5c2949d363b3da4265ce6c85facae134e81e0dab783b83bb7c761dba4c2db9f2
# {
#  "hash": "5c2949d363b3da4265ce6c85facae134e81e0dab783b83bb7c761dba4c2db9f2",
#  "confirmations": 101,
#  "height": 625,
#  "version": 536870912,
#  "versionHex": "20000000",
#  "merkleroot": "a743cd8c1ab28cba64a7074bb53449064d6ccdc17a1201641f03f33a70e562d1",
#  "time": 1755519738,
#  "mediantime": 1755519359,
#  "nonce": 0,
#  "bits": "207fffff",
#  "target": "7fffff0000000000000000000000000000000000000000000000000000000000",
#  "difficulty": 4.656542373906925e-10,
#  "chainwork": "00000000000000000000000000000000000000000000000000000000000004e4",
#  "nTx": 1,
#  "previousblockhash": "15a37423af57235d8194f64c1f9a1f5330effff6d3c2963d2556be3eece39e22",
#  "nextblockhash": "798d4206f1547970829c1484b380753c03d5a79c7703ae5601fe38e6a2100dd2",
#  "strippedsize": 226,
#  "size": 262,
#  "weight": 940,
#  "tx": [
#    "a743cd8c1ab28cba64a7074bb53449064d6ccdc17a1201641f03f33a70e562d1"
#  ]
# }

bitcoin-cli -rpcconnect=127.0.0.1 -rpcport=18443 -rpcpassword=bitcoinpass -rpcuser=bitcoin getrawtransaction a743cd8c1ab28cba64a7074bb53449064d6ccdc17a1201641f03f33a70e562d1
#020000000001010000000000000000000000000000000000000000000000000000000000000000ffffffff0402710200ffffffff02205fa01200000000225120c55954de9824438180cc265fb53d024a0d0b4e81a003bbc2c826e3b69f71c1a80000000000000000266a24aa21a9ede2f61c3f71d1defd3fa999dfa36953755c690689799962b48bebd836974e8cf90120000000000000000000000000000000000000000000000000000000000000000000000000
# 
# when we parse it on https://www.blockchain.com/explorer/assets/btc/decode-transaction: 
# {"version":2,"locktime":0,"ins":[{"n":4294967295,"script":{"asm":"7102 OP_0","hex":"02710200"},"sequence":4294967295,"txid":"0000000000000000000000000000000000000000000000000000000000000000","witness":["0000000000000000000000000000000000000000000000000000000000000000"]}],"outs":[{"n":0,"script":{"addresses":[],"asm":"OP_1 c55954de9824438180cc265fb53d024a0d0b4e81a003bbc2c826e3b69f71c1a8","hex":"5120c55954de9824438180cc265fb53d024a0d0b4e81a003bbc2c826e3b69f71c1a8"},"value":312500000},{"n":1,"script":{"addresses":[],"asm":"OP_RETURN aa21a9ede2f61c3f71d1defd3fa999dfa36953755c690689799962b48bebd836974e8cf9","hex":"6a24aa21a9ede2f61c3f71d1defd3fa999dfa36953755c690689799962b48bebd836974e8cf9"},"value":0}],"hash":"a743cd8c1ab28cba64a7074bb53449064d6ccdc17a1201641f03f33a70e562d1","txid":"a743cd8c1ab28cba64a7074bb53449064d6ccdc17a1201641f03f33a70e562d1"}
```
