use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 0,
    Uninitialized = 1,
    Paused = 2,
    UnsupportedToken = 3,
    AlreadySupportedToken = 4,
    NotEnoughBalance = 5,
    NotEnoughCollateral = 6,
    NotEnoughLiquidity = 7,
    NotOverLiquidationThreshold = 8,
    MustNotHaveBorrow = 9,
}
