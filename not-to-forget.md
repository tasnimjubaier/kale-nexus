

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



CARGO_TARGET_DIR=contracts/oracle_adapter/target \
cargo build -p oracle_adapter --target wasm32v1-none --release


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






# call relfector

stellar contract invoke --id CAVLP5DH2GJPZMVO7IJY4CVOD5MWEFTJFVPD2YY2FQXOQHRGHK4D6HLP --source-account alice \
  --rpc-url https://soroban-testnet.stellar.org \
  --network-passphrase "Test SDF Network ; September 2015" \
  --send=no \
   --asset BTC --override-max-age-secs null

stellar contract invoke --id CAVLP5DH2GJPZMVO7IJY4CVOD5MWEFTJFVPD2YY2FQXOQHRGHK4D6HLP --source-account alice \
--rpc-url https://soroban-testnet.stellar.org --network-passphrase Test SDF Network ; September 2015 --send=no -- [COMMAND]

stellar contract invoke \
  --id CCSSOHTBL3LEWUCBBEB5NJFC2OKFRC74OWEIJIZLRJBGAAU4VMU5NV4W \
  --source alice -n testnet -- \
  assets






# settings

export CONTRACT=CCMWA6KUHQWA5XJGV4CG6ELEVETHRBQDGDCRSD7HCG5FEEGQZ5NXRUC5   
export SRC=alice
export ADMIN=$(stellar keys public-key $SRC)
export REFLECTOR=CAVLP5DH2GJPZMVO7IJY4CVOD5MWEFTJFVPD2YY2FQXOQHRGHK4D6HLP
export REFLECTOR=CALI2BYU2JE6WVRUFYTS6MSBNEHGJ35P4AVCZYF3B6QOE3QKOB2PLE6M




# invocation 
stellar contract invoke \
  --id $CONTRACT --source-account $SRC --network testnet \
  -- init \
  --admin $ADMIN \
  --reflector $REFLECTOR



stellar contract invoke \
  --id $CONTRACT --source-account $SRC --network testnet  \
  -- set_feeder \
  --caller $ADMIN \
  --feeder $ADMIN



stellar contract invoke \
  --id $CONTRACT --network testnet --source-account $SRC \
  -- set_reflector \
  --caller $ADMIN \
  --reflector $REFLECTOR


stellar contract invoke \
  --id $CONTRACT --network testnet --source-account $SRC \
  -- upsert_asset \
  --caller $ADMIN \
  --asset-code BTC \
  --decimals 8 \
  --max-age-secs 120


---------

stellar contract invoke \
  --id $CONTRACT --source-account $SRC --network testnet \
  -- pull_from_reflector \
  --caller $ADMIN \
  --asset '{"Other":"KALE"}'

stellar contract invoke \
  --id $CONTRACT --source-account $SRC --network testnet \
  -- get_spot \
  --asset BTC 



stellar contract invoke \
  --id $CONTRACT --source-account $SRC --network testnet \
  -- get_twap \
  --caller $ADMIN \
  --asset BTC --records 12

stellar contract invoke \
  --id $CONTRACT --source-account $SRC --network testnet \
  -- push_price \
  --caller $ADMIN \
  --asset BTC --price 6500000000000 --decimals 8








[
  {
    "udt_union_v0": {
      "doc": "",
      "lib": "",
      "name": "DataKey",
      "cases": [
        {
          "void_v0": {
            "doc": "",
            "name": "Admin"
          }
        },
        {
          "void_v0": {
            "doc": "",
            "name": "Feeder"
          }
        },
        {
          "void_v0": {
            "doc": "",
            "name": "Reflector"
          }
        },
        {
          "tuple_v0": {
            "doc": "",
            "name": "AssetCfg",
            "type_": [
              "string"
            ]
          }
        },
        {
          "tuple_v0": {
            "doc": "",
            "name": "History",
            "type_": [
              "string"
            ]
          }
        }
      ]
    }
  },
  {
    "udt_struct_v0": {
      "doc": "",
      "lib": "",
      "name": "AssetCfg",
      "fields": [
        {
          "doc": "",
          "name": "decimals",
          "type_": "u32"
        },
        {
          "doc": "",
          "name": "max_age_secs",
          "type_": "u64"
        }
      ]
    }
  },
  {
    "udt_struct_v0": {
      "doc": "",
      "lib": "",
      "name": "PricePoint",
      "fields": [
        {
          "doc": "",
          "name": "decimals",
          "type_": "u32"
        },
        {
          "doc": "",
          "name": "price",
          "type_": "i128"
        },
        {
          "doc": "",
          "name": "ts",
          "type_": "u64"
        }
      ]
    }
  },
  {
    "udt_union_v0": {
      "doc": "",
      "lib": "",
      "name": "Asset",
      "cases": [
        {
          "tuple_v0": {
            "doc": "",
            "name": "Other",
            "type_": [
              "string"
            ]
          }
        },
        {
          "tuple_v0": {
            "doc": "",
            "name": "Stellar",
            "type_": [
              "string",
              "address"
            ]
          }
        }
      ]
    }
  },
  {
    "udt_error_enum_v0": {
      "doc": "",
      "lib": "",
      "name": "Err",
      "cases": [
        {
          "doc": "",
          "name": "NotInitialized",
          "value": 1
        },
        {
          "doc": "",
          "name": "AlreadyInitialized",
          "value": 2
        },
        {
          "doc": "",
          "name": "NotAdmin",
          "value": 3
        },
        {
          "doc": "",
          "name": "NotFeeder",
          "value": 4
        },
        {
          "doc": "",
          "name": "UnknownAsset",
          "value": 5
        },
        {
          "doc": "",
          "name": "StalePrice",
          "value": 6
        },
        {
          "doc": "",
          "name": "NoHistory",
          "value": 7
        },
        {
          "doc": "",
          "name": "BadDecimals",
          "value": 8
        },
        {
          "doc": "",
          "name": "MathOverflow",
          "value": 9
        },
        {
          "doc": "",
          "name": "ReflectorNotSet",
          "value": 10
        }
      ]
    }
  },
  {
    "function_v0": {
      "doc": "",
      "name": "init",
      "inputs": [
        {
          "doc": "",
          "name": "admin",
          "type_": "address"
        },
        {
          "doc": "",
          "name": "reflector",
          "type_": "address"
        }
      ],
      "outputs": []
    }
  },
  {
    "function_v0": {
      "doc": "",
      "name": "set_feeder",
      "inputs": [
        {
          "doc": "",
          "name": "caller",
          "type_": "address"
        },
        {
          "doc": "",
          "name": "feeder",
          "type_": "address"
        }
      ],
      "outputs": []
    }
  },
  {
    "function_v0": {
      "doc": "",
      "name": "set_reflector",
      "inputs": [
        {
          "doc": "",
          "name": "caller",
          "type_": "address"
        },
        {
          "doc": "",
          "name": "reflector",
          "type_": "address"
        }
      ],
      "outputs": []
    }
  },
  {
    "function_v0": {
      "doc": "",
      "name": "upsert_asset",
      "inputs": [
        {
          "doc": "",
          "name": "caller",
          "type_": "address"
        },
        {
          "doc": "",
          "name": "asset_code",
          "type_": "string"
        },
        {
          "doc": "",
          "name": "decimals",
          "type_": "u32"
        },
        {
          "doc": "",
          "name": "max_age_secs",
          "type_": "u64"
        }
      ],
      "outputs": []
    }
  },
  {
    "function_v0": {
      "doc": "",
      "name": "pull_from_reflector",
      "inputs": [
        {
          "doc": "",
          "name": "caller",
          "type_": "address"
        },
        {
          "doc": "",
          "name": "asset",
          "type_": {
            "udt": {
              "name": "Asset"
            }
          }
        }
      ],
      "outputs": [
        {
          "udt": {
            "name": "PricePoint"
          }
        }
      ]
    }
  },
  {
    "function_v0": {
      "doc": "",
      "name": "push_price",
      "inputs": [
        {
          "doc": "",
          "name": "caller",
          "type_": "address"
        },
        {
          "doc": "",
          "name": "asset_code",
          "type_": "string"
        },
        {
          "doc": "",
          "name": "price",
          "type_": "i128"
        },
        {
          "doc": "",
          "name": "decimals",
          "type_": "u32"
        },
        {
          "doc": "",
          "name": "ts",
          "type_": "u64"
        }
      ],
      "outputs": []
    }
  },
  {
    "function_v0": {
      "doc": "",
      "name": "get_spot",
      "inputs": [
        {
          "doc": "",
          "name": "asset_code",
          "type_": "string"
        },
        {
          "doc": "",
          "name": "out_decimals",
          "type_": "u32"
        }
      ],
      "outputs": [
        {
          "tuple": {
            "value_types": [
              "i128",
              "u32",
              "u64"
            ]
          }
        }
      ]
    }
  },
  {
    "function_v0": {
      "doc": "",
      "name": "get_twap",
      "inputs": [
        {
          "doc": "",
          "name": "asset_code",
          "type_": "string"
        },
        {
          "doc": "",
          "name": "records",
          "type_": "u32"
        },
        {
          "doc": "",
          "name": "out_decimals",
          "type_": "u32"
        }
      ],
      "outputs": [
        {
          "tuple": {
            "value_types": [
              "i128",
              "u32",
              "u64"
            ]
          }
        }
      ]
    }
  }
]