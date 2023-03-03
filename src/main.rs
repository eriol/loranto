mod ble;
mod utils;

use std::error::Error;

use clap::{Parser, Subcommand};

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
    /// Send messages to a device
    Send {
        /// device's address
        #[arg(long)]
        device: String,
        #[arg(required = true)]
        text: Vec<String>,
    },
    /// Send messages to a device
    Repl {
        /// device's address
        #[arg(long)]
        device: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();

    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Scan { scan_time }) => {
            let devices = ble::scan(cli.adapter, *scan_time).await?;

            for device in devices {
                println!(
                    "{} (rssi:{}) {}",
                    device.address, device.rssi, device.local_name
                );
            }
        }
        Some(Commands::Send { device, text }) => {
            ble::send(cli.adapter, device.clone(), text.join(" ")).await?;
        }
        Some(Commands::Repl { device }) => {
            ble::repl(cli.adapter, device.clone()).await?;
        }
        None => {}
    }

    Ok(())
}
