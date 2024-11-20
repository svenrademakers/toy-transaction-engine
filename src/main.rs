use crossbeam::channel::bounded;
use csv_source::{run_csv_source, write_accounts_to_csv};
use transaction_processor::TransactionProcessor;

mod csv_source;
mod data_types;
mod transaction_context;
mod transaction_processor;

fn main() -> anyhow::Result<()> {
    // number is arbitrary guesstimate depending on incoming volume
    let (sender, receiver) = bounded(1024 * 1024);

    // source can be anything that produces [`TransactionEvent`] data.
    run_csv_source(sender)?;

    // run engine
    let accounts = TransactionProcessor::exhaust_sources(receiver);

    write_accounts_to_csv(accounts)
}
