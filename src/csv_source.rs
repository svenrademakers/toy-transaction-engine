use crate::data_types::{Account, TransactionEvent};
use anyhow::bail;
use csv::{ReaderBuilder, Writer};
use rtrb::Producer;
use std::env;

/// non-blocking task that reads csv data on a seperate thread and sends it over a channel
pub fn run_csv_source(mut producer: Producer<TransactionEvent>) -> anyhow::Result<()> {
    let Some(file_path) = env::args().nth(1) else {
        bail!("Usage: {} <file_path>", env!("CARGO_PKG_NAME"))
    };

    let mut rdr = ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::Fields)
        .from_path(file_path)?;

    std::thread::Builder::new()
        .name("CSV source".to_string())
        .spawn(move || {
            for transaction in rdr.deserialize().filter_map(|item| item.ok()) {
                producer.push(transaction).expect("CSV source died");
            }
        })?;

    Ok(())
}

pub fn write_accounts_to_csv(accounts: impl Iterator<Item = (u16, Account)>) -> anyhow::Result<()> {
    let mut writer = Writer::from_writer(std::io::stdout());
    writer.write_record(["client", "available", "held", "total", "locked"])?;

    for (client_id, account) in accounts {
        writer.write_record(&[
            client_id.to_string(),
            account.available().to_string(),
            account.held.to_string(),
            account.total.to_string(),
            account.locked.to_string(),
        ])?;
    }

    Ok(writer.flush()?)
}
