use soroban_sdk::contracttype;

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    LendingContract,
    MarginPositionsContract,
    Admin,
}
