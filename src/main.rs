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
    /// Scan to find BLE devices.
    Scan {
        /// scan duration
        #[arg(long, default_value_t = 5.0)]
        scan_time: f32,
    },
}

fn main() {
    let cli = Cli::parse();

    println!("Value for adapter: {}", cli.adapter);
}
