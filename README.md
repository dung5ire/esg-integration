
### Build


```sh
cargo build --release
```



### Single-Node Development Chain

This command will start the single-node development chain

```bash
./target/release/node-template --dev
```

# API

## Get ESG score seperately


- *company*: Name of the company

- Returns `sustainable_score` on-chain for specific company

## How we get `sustainable score` on-chain 

1. Insert key for specific company ( Each company have own private key and private key)

```json

//REQUEST:

curl http://localhost:9933 -H "Content-Type:application/json;charset=utf-8" -d   '{ "jsonrpc":"2.0", "id":1, "method":"author_insertKey", "params": ["esg!","//Alice","0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"] }'

//RESPONSE:

{"jsonrpc":"2.0","result":null,"id":1}
```

2. Choose your company to get true endpoint


3. Get sustainble score
```bash
./scripts/get_esg.sh -c <company name>
```
### Example company name
- facebook
- alphabet
- microsoft

```json

// RESPONSE:

{"jsonrpc":"2.0","result":312312,"id":1}

```
