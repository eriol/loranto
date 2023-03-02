use std::error::Error;
use std::time::Duration;

use btleplug::api::{BDAddr, Central, Manager as _, Peripheral, ScanFilter};
use btleplug::platform::{Adapter, Manager};
use tokio::time;
use uuid::Uuid;

use crate::utils::progress_bar;

const NORDIC_UART_SERVICE_UUID: Uuid = Uuid::from_u128(0x6e400001_b5a3_f393_e0a9_e50e24dcca9e);
const INVALID_RSSI: i16 = i16::MIN;

/// A result from Bluetooth scan.
#[derive(Debug, Default, Clone)]
pub struct ScanResult {
    pub address: BDAddr,
    pub local_name: String,
    pub rssi: i16,
}

pub async fn scan(adapter_name: String, scan_time: u64) -> Result<Vec<ScanResult>, Box<dyn Error>> {
    let scan_time = Duration::from_secs(scan_time);
    let manager = Manager::new().await?;
    let adapter = get_adapter_by_name(&manager, adapter_name).await?;
    adapter
        .start_scan(ScanFilter::default())
        .await
        .expect("An error occurred while scanning for devices");

    progress_bar(scan_time);
    time::sleep(scan_time).await;

    let mut results: Vec<ScanResult> = Vec::new();
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
                .unwrap_or(INVALID_RSSI);
            let local_name = properties
                .as_ref()
                .ok_or_else(|| "Error reading device name".to_string())?
                .local_name
                .clone()
                .unwrap_or(address.to_string());
            results.push(ScanResult {
                address,
                local_name,
                rssi,
            });
        }
    }

    results.sort_by(|a, b| b.rssi.cmp(&a.rssi));
    Ok(results)
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
