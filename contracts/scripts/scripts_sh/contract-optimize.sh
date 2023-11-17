cargo build --target wasm32-unknown-unknown --release

wait

soroban contract optimize --wasm target/wasm32-unknown-unknown/release/lending.wasm