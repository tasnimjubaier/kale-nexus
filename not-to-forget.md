

# stellar key generate and see private key: 


stellar keys generate --global alice --network testnet --fund

stellar keys address alice

stellar keys show alice



# contract list

stellar contract list --network testnet


# deploy
stellar contract deploy \
  --wasm target/wasm32v1-none/release/round_engine.wasm \
  --source-account alice --network testnet --alias round_engine

stellar contract deploy \
  --wasm target/wasm32v1-none/release/oracle_adapter.wasm \
  --source-account alice --network testnet --alias oracle_adapter

stellar contract deploy \
  --wasm target/wasm32v1-none/release/kale_pass_treasury.wasm \
  --source-account alice --network testnet --alias kale_pass_treasury

stellar contract deploy \
  --wasm target/wasm32v1-none/release/creator_hub.wasm \
  --source-account alice --network testnet --alias creator_hub




# Generate TypeScript bindings (for the web app)


stellar contract bindings typescript \
  --network testnet \
  --contract-id round_engine \
  --output-dir packages/round_engine

cd packages/round_engine
npm install && npm run build
cd ../../



stellar contract bindings typescript \
  --network testnet \
  --contract-id kale_pass_treasury \
  --output-dir packages/kale_pass_treasury

cd packages/kale_pass_treasury
npm install && npm run build
cd ../../



stellar contract bindings typescript \
  --network testnet \
  --contract-id creator_hub \
  --output-dir packages/creator_hub

cd packages/creator_hub
npm install && npm run build
cd ../../



# Spin up the frontend

npm create next-app@latest frontend --ts --eslint
cd frontend