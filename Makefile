build:
\tstellar contract build

deploy:
\tstellar contract deploy --wasm target/wasm32v1-none/release/round_engine.wasm \
\t  --source-account alice --network testnet --alias round_engine
\t
\tstellar contract deploy --wasm target/wasm32v1-none/release/oracle_adapter.wasm \
\t  --source-account alice --network testnet --alias oracle_adapter
\t
\tstellar contract deploy --wasm target/wasm32v1-none/release/kale_pass_treasury.wasm \
\t  --source-account alice --network testnet --alias kale_pass_treasury
\t
\tstellar contract deploy --wasm target/wasm32v1-none/release/creator_hub.wasm \
\t  --source-account alice --network testnet --alias creator_hub

bindings:
\tstellar contract bindings typescript --network testnet \
\t  --contract-id round_engine --output-dir packages/round_engine
\tcd packages/round_engine && npm i && npm run build
