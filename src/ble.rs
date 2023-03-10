// Copyright © 2023 Daniele Tricoli <eriol@mornie.org>
// SPDX-License-Identifier: BSD-3-Clause

use std::error::Error;
use std::str;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use btleplug::api::{
    BDAddr, Central, CentralEvent, Manager as _, Peripheral as _, ScanFilter, WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral};
use clap::{crate_name, crate_version};
use console::Term;
use futures::stream::StreamExt;
use tokio::time;
use uuid::Uuid;

use crate::utils::{get_stdin_line_channel, progress_bar};

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
        // We don't specify a scan filter because the paired devices are showed
        // anyway.
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
    term.write_line(format!("{} {}", crate_name!(), crate_version!()).as_str())?;
    term.write_line(format!("Connecting to... {}", address).as_str())?;
    let device = find_device_by_address(adapter_name, address).await?;
    device.connect().await?;
    if device.is_connected().await? {
        term.write_line("Connected. Type quit() to exit.")?;
        device.discover_services().await?;

        let line_channel = get_stdin_line_channel();
        tokio::spawn(write_ble(device.clone(), line_channel));

        // Receive data from the Bluetooth LE device.
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
    }
    Ok(())
}

/// Send data to Bluetooth LE device.
async fn write_ble(device: Peripheral, text_channel: Receiver<String>) {
    let chars = device.characteristics();
    let tx_char = chars
        .iter()
        .find(|c| c.uuid == NORDIC_UART_TX_CHAR_UUID)
        .ok_or("Unable to find TX characteric")
        .unwrap();
    loop {
        let mut words = String::new();
        if let Ok(text) = text_channel.try_recv() {
            words = text.trim().to_string();
        }
        if !words.is_empty() {
            if words == "quit()" {
                device
                    .disconnect()
                    .await
                    .expect("Error disconnect from device.");
                std::process::exit(0);
            }
            device
                .write(&tx_char, words.as_bytes(), WriteType::WithoutResponse)
                .await
                .unwrap();
        }
        time::sleep(Duration::from_millis(100)).await;
    }
}
