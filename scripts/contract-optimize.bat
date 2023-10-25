cargo build --target wasm32-unknown-unknown --release --no-default-features
soroban contract optimize --wasm target\wasm32-unknown-unknown\release\soroban_lending.wasm