
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

`Get /esg_score/{company}`

- *company*: Name of the company

- Returns `sustainable_score` on-chain for specific company

## Example

```bash
./scripts/get_esg.sh -c <company name>
```

// RESPONSE:
{"jsonrpc":"2.0","result":<`sustainable score`,"id":1}
