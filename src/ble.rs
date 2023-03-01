use std::error::Error;
use std::time::Duration;
use tokio::time;
use uuid::Uuid;

use btleplug::api::{Central, Manager as _, Peripheral, ScanFilter};
use btleplug::platform::{Adapter, Manager};

const NORDIC_UART_SERVICE_UUID: Uuid = Uuid::from_u128(0x6e400001_b5a3_f393_e0a9_e50e24dcca9e);

pub async fn scan(adapter_name: String, scan_time: u64) -> Result<(), Box<dyn Error>> {
    let manager = Manager::new().await?;
    let adapter = get_adapter_by_name(&manager, adapter_name).await?;
    adapter
        .start_scan(ScanFilter::default())
        .await
        .expect("An error occurred while scanning for devices");

    time::sleep(Duration::from_secs(scan_time)).await;

    let peripherals = adapter.peripherals().await?;
    if peripherals.is_empty() {
        eprintln!("No devices found");
    } else {
        for peripheral in peripherals.iter() {
            let properties = peripheral.properties().await?;
            let services = &properties
                .as_ref()
                .ok_or_else(|| "Error discovering services".to_string())?
                .services;
            if !services.contains(&NORDIC_UART_SERVICE_UUID) {
                continue;
            }
            let address = properties
                .as_ref()
                .ok_or_else(|| "Error reading device address".to_string())?
                .address;
            let rssi = properties
                .as_ref()
                .ok_or_else(|| "Error reading device rssi".to_string())?
                .rssi
                .unwrap_or(0);
            let name = properties
                .as_ref()
                .ok_or_else(|| "Error reading device name".to_string())?
                .local_name
                .clone()
                .unwrap_or(address.to_string());
            println!("{} (rssi:{}) {}", address, rssi, name);
        }
    }

    Ok(())
}

async fn get_adapter_by_name(manager: &Manager, name: String) -> Result<Adapter, Box<dyn Error>> {
    let adapters = manager.adapters().await?;
    for adapter in adapters {
        if adapter.adapter_info().await?.contains(&name) {
            return Ok(adapter);
        }
    }

    Err(format!("Can't find adapter: {}", name).into())
}
