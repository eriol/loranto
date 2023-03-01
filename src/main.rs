mod ble;

use clap::{Parser, Subcommand};
use std::error::Error;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(long, default_value = "hci0")]
    adapter: String,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan to find Bluetooth LE devices.
    Scan {
        /// scan duration
        #[arg(long, default_value_t = 5)]
        scan_time: u64,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Scan { scan_time }) => {
            ble::scan(cli.adapter, *scan_time).await?;
        }
        None => {}
    }

    Ok(())
}
