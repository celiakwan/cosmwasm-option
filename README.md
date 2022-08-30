# cosmwasm-option
A smart contract written in Rust and built with CosmWasm for options trading.

### Version
- [Rust](https://www.rust-lang.org/): 1.61.0
- [CosmWasm](https://cosmwasm.com/): 1.0.0
- [wasmd](https://github.com/CosmWasm/wasmd): 0.23.0

### Installation
Install Rust.
```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Build
```
cargo build
```

### Compile
```
docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/rust-optimizer:0.12.6
```

### Schema
Generate JSON schema files. The files will be saved to the `schema` folder.
```
cargo schema
```

### Get Started
1. Set up parameters.
```
export CHAIN_ID="cliffnet-1"
export RPC="https://rpc.cliffnet.cosmwasm.com:443"
export FAUCET="https://faucet.cliffnet.cosmwasm.com"
export NODE=(--node $RPC)
export TXFLAG=($NODE --chain-id $CHAIN_ID --gas-prices 0.025upebble --gas auto --gas-adjustment 1.3)
```

2. Create wallets.
```
wasmd keys add wallet1
wasmd keys add wallet2
WALLET1=$(wasmd keys show -a wallet1)
WALLET2=$(wasmd keys show -a wallet2)
```

3. Request tokens from faucet.
```
JSON=$(jq -n --arg addr $WALLET1 '{"denom":"upebble","address":$addr}') && curl -X POST --header "Content-Type: application/json" --data "$JSON" "$FAUCET"/credit
JSON=$(jq -n --arg addr $WALLET2 '{"denom":"upebble","address":$addr}') && curl -X POST --header "Content-Type: application/json" --data "$JSON" "$FAUCET"/credit
```

4. Upload the binary to the chain.
```
RES=$(wasmd tx wasm store artifacts/cosmwasm_option.wasm --from wallet1 $TXFLAG -y --output json -b block)
```

5. Get the code ID of the uploaded binary.
```
CODE_ID=$(echo $RES | jq -r '.logs[0].events[-1].attributes[0].value')
```

6. Create an instance of the contract.
```
INIT='{"counter_offer":[{"amount":"10","denom":"upebble"}],"expires":2180481}'
wasmd tx wasm instantiate $CODE_ID "$INIT" \
    --amount 5upebble --from wallet1 --label "cosmwasm option example" \
    $TXFLAG -y --no-admin
```

7. Query contract state addresses and save the latest one to `CONTRACT`.
```
CONTRACT=$(wasmd query wasm list-contract-by-code $CODE_ID $NODE --output json | jq -r '.contracts[-1]')
```

8. Query the contract info and balance.
```
wasmd query wasm contract $CONTRACT $NODE
wasmd query bank balances $CONTRACT $NODE
```

9. Query the contract state by the state address.
```
wasmd query wasm contract-state smart $CONTRACT '"config"' $NODE
```

10. Transfer the option from wallet1 to wallet2.
```
TRANSFER='{"transfer":{"recipient":"'WALLET2'"}}'
wasmd tx wasm execute $CONTRACT "$TRANSFER" \
    --from wallet1 $TXFLAG -y
```

11. Finalize the trade.
```
FINALIZE='"finalize"'
wasmd tx wasm execute $CONTRACT "$FINALIZE" \
    --amount 10upebble --from wallet2 $TXFLAG -y
```

12. Query the contract state. This should fail since the state should has been removed.
```
wasmd query wasm contract-state smart $CONTRACT '"config"' $NODE
```

13. Create another instance of the contract.
```
INIT='{"counter_offer":[{"amount":"10","denom":"upebble"}],"expires":2112172}'
wasmd tx wasm instantiate $CODE_ID "$INIT" \
    --amount 5upebble --from wallet1 --label "cosmwasm option example" \
    $TXFLAG -y --no-admin
```

14. Update `CONTRACT`.
```
CONTRACT=$(wasmd query wasm list-contract-by-code $CODE_ID $NODE --output json | jq -r '.contracts[-1]')
```

15. Query the contract state by the state address.
```
wasmd query wasm contract-state smart $CONTRACT '"config"' $NODE
```

16. Burn the option.
```
BURN='"burn"'
wasmd tx wasm execute $CONTRACT "$BURN" \
    --from wallet1 $TXFLAG -y
```

17. Query the contract state. This should fail since the state should has been removed.
```
wasmd query wasm contract-state smart $CONTRACT '"config"' $NODE
```

### Testing
```
cargo test
```

### Reference
- https://docs.cosmwasm.com/tutorials/simple-option/intro/