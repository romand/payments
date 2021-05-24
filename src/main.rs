#[cfg(test)]
#[macro_use]
extern crate quickcheck;

mod amount;
mod process;
mod tx;

use process::*;
use std::io;

fn main() -> Result<(), csv::Error> {
    let input_path = std::env::args().nth(1).expect("no path to input given");

    let mut tx_proc = TxProcessor::new();
    let mut rdr = csv::Reader::from_path(input_path)?;
    for tx in rdr.deserialize() {
        match tx {
            Ok(tx) => {
                if let Err(err) = tx_proc.process(&tx) {
                    eprintln!("failed to process {:?}: {}", tx, err)
                }
            }
            Err(err) => {
                eprintln!("failed to parse tx: {}", err)
            }
        }
    }

    let mut wtr = csv::Writer::from_writer(io::stdout());
    wtr.write_record(&["client", "available", "held", "total", "locked"])?;
    for ClientSummary {
        id,
        available,
        held,
        total,
        locked,
    } in tx_proc.client_summaries()
    {
        wtr.serialize((id, available, held, total, locked))?
    }

    Ok(())
}
