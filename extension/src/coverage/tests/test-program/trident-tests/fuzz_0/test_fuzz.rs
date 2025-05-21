use fuzz_transactions::*;
use trident_fuzz::fuzzing::*;
mod fuzz_transactions;
mod instructions;
mod transactions;
mod types;
use test_program::entry as entry_test_program;
pub use transactions::*;
#[derive(Default)]
struct FuzzTest<C> {
    client: C,
}
/// Use flows to specify custom sequences of behavior
/// #[init]
/// fn start(&mut self) {
///     // Initialization goes here
/// }
/// #[flow]
/// fn flow1(
///     &mut self,
///     fuzzer_data: &mut FuzzerData,
///     accounts: &mut FuzzAccounts,
/// ) -> Result<(), FuzzingError> {
///     // Flow logic goes here
///     Ok(())
/// }
#[flow_executor]
impl<C: FuzzClient + std::panic::RefUnwindSafe> FuzzTest<C> {
    fn new(client: C) -> Self {
        Self { client }
    }
    #[init]
    fn start(&mut self) {
        self.client.deploy_entrypoint(TridentEntrypoint::new(
            pubkey!("W7GzUqFd8a1b7AEn4jBDjd2T1p6QzyqunxXvcvf82PM"),
            None,
            processor!(entry_test_program),
        ));
    }
}
fn main() {
    let client = TridentSVM::new_client(&TridentConfig::new());
    FuzzTest::new(client).fuzz();
}
