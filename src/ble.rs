use std::error::Error;
use std::str;
use std::time::Duration;

use btleplug::api::{
    BDAddr, Central, CentralEvent, Manager as _, Peripheral as _, ScanFilter, WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral};
use console::Term;
use dialoguer::{theme::ColorfulTheme, Input};
use futures::stream::StreamExt;
use tokio::time;
use uuid::Uuid;

use crate::utils::progress_bar;

const NORDIC_UART_SERVICE_UUID: Uuid = Uuid::from_u128(0x6e400001_b5a3_f393_e0a9_e50e24dcca9e);
const NORDIC_UART_TX_CHAR_UUID: Uuid = Uuid::from_u128(0x6e400002_b5a3_f393_e0a9_e50e24dcca9e);
const NORDIC_UART_RX_CHAR_UUID: Uuid = Uuid::from_u128(0x6e400003_b5a3_f393_e0a9_e50e24dcca9e);
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

async fn find_device_by_address(
    adapter_name: String,
    address: String,
) -> Result<Peripheral, Box<dyn Error>> {
    let manager = Manager::new().await?;
    let adapter = get_adapter_by_name(&manager, adapter_name).await?;

    let mut events = adapter.events().await?;
    adapter.start_scan(ScanFilter::default()).await?;

    while let Some(event) = events.next().await {
        match event {
            CentralEvent::DeviceDiscovered(_id) => {
                let peripherals = adapter.peripherals().await?;
                for peripheral in peripherals.iter() {
                    let properties = peripheral.properties().await?;
                    let device_address = properties
                        .as_ref()
                        .ok_or_else(|| "Error reading device address".to_string())?
                        .address;
                    if device_address.to_string() == address {
                        return Ok(peripheral.clone());
                    }
                }
            }
            _ => {}
        }
    }

    Err("no device found".into())
}

pub async fn send(
    adapter_name: String,
    address: String,
    text: String,
) -> Result<(), Box<dyn Error>> {
    let is_a_command = text.starts_with("!");

    let device = find_device_by_address(adapter_name, address).await?;
    device.connect().await?;
    if device.is_connected().await? {
        device.discover_services().await?;
        let chars = device.characteristics();
        let tx_char = chars
            .iter()
            .find(|c| c.uuid == NORDIC_UART_TX_CHAR_UUID)
            .ok_or("Unable to find TX characteric")?;

        if is_a_command {
            let rx_char = chars
                .iter()
                .find(|c| c.uuid == NORDIC_UART_RX_CHAR_UUID)
                .ok_or("Unable to find RX characteric")?;
            device.subscribe(&rx_char).await?;
        }
        let type_ = if is_a_command {
            WriteType::WithResponse
        } else {
            WriteType::WithoutResponse
        };
        device.write(&tx_char, text.as_bytes(), type_).await?;
        if is_a_command {
            let mut notification_stream = device.notifications().await?.take(1);
            while let Some(data) = notification_stream.next().await {
                let text = str::from_utf8(&data.value)?;
                println!("{}", text.trim_end());
            }
        }

        device.disconnect().await?;
    }

    Ok(())
}

pub async fn repl(adapter_name: String, address: String) -> Result<(), Box<dyn Error>> {
    let term = Term::stdout();
    let device = find_device_by_address(adapter_name, address).await?;
    device.connect().await?;
    if device.is_connected().await? {
        device.discover_services().await?;
        tokio::spawn(get_input(device.clone(), term.clone()));
        let chars = device.characteristics();
        let rx_char = chars
            .iter()
            .find(|c| c.uuid == NORDIC_UART_RX_CHAR_UUID)
            .ok_or("Unable to find RX characteric")?;
        device.subscribe(&rx_char).await?;
        let mut notification_stream = device.notifications().await?;
        while let Some(data) = notification_stream.next().await {
            let text = str::from_utf8(&data.value)?;
            term.write_line(text.trim_end())?;
            term.flush()?;
        }
        device.disconnect().await?;
    }
    Ok(())
}

async fn get_input(device: Peripheral, t: Term) {
    let chars = device.characteristics();
    let tx_char = chars
        .iter()
        .find(|c| c.uuid == NORDIC_UART_TX_CHAR_UUID)
        .ok_or("Unable to find TX characteric")
        .unwrap();
    loop {
        let text: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Φ]")
            .interact_on(&t)
            .unwrap();
        device
            .write(&tx_char, text.as_bytes(), WriteType::WithoutResponse)
            .await
            .unwrap();
        time::sleep(Duration::from_millis(100)).await;
    }
}
