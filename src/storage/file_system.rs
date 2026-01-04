use embassy_rp::{gpio::Output, rom_data, Peri};
use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, signal::Signal};

use embassy_time::{block_for, Duration, Timer};
use heapless::{String, Vec};
use littlefs2::{
    fs::{Allocation, File, Filesystem, OpenOptions},
    path,
};
use littlefs2_core::{Error, Path, SeekFrom};

use crate::{
    artnet_task::ARTNET_NODE_CONFIG,
    dmx_task::DMX_PORT_CONFIG,
    log::{self, try_log, try_log_debug},
    schema::{SETTINGS_VERSION, StoredSettings},
    storage::flash_storage_adapter::FlashStorage,
    web_task::NETWORK_NODE_CONFIG,
};

#[derive(PartialEq, Eq)]
pub enum SettingsStoreSignal {
    Save,
    SaveAndReboot,
}

pub static SETTINGS_STORE_SIGNAL: Signal<ThreadModeRawMutex, SettingsStoreSignal> = Signal::new();
const SETTINGS_PATH: &Path = path!("settings.json");

#[embassy_executor::task]
pub async fn file_system_task(
    flash: Peri<'static, embassy_rp::peripherals::FLASH>,
    dma: Peri<'static, embassy_rp::peripherals::DMA_CH2>,
) {
    log::log("[STORAGE] File system task starting").await;
    let mut storage = crate::storage::flash_storage_adapter::FlashStorage::new(
        embassy_rp::flash::Flash::new(flash, dma),
    );
    let mut alloc = Filesystem::allocate();

    // Try to mount first - if it fails, format and try again
    let fs = match Filesystem::mount(&mut alloc, &mut storage) {
        Ok(fs) => fs,
        Err(_) => {
            // Mount failed, format the filesystem
            if let Err(e) = Filesystem::format(&mut storage) {
                try_log("[STORAGE] Failed to format filesystem");
                try_log_debug(&e);
                return; // Exit if format fails
            }

            // Try mounting again after format
            match Filesystem::mount(&mut alloc, &mut storage) {
                Ok(fs) => fs,
                Err(e) => {
                    try_log("[STORAGE] Failed to mount filesystem");
                    try_log_debug(&e);
                    return; // Exit if mount fails after format
                }
            }
        }
    };
    try_log("[STORAGE] Filesystem mounted successfully. Free space:");
    try_log_debug(&fs.available_space().unwrap());

    load_settings(&fs).await;

    // Wait for settings store signal and save settings to filesystem
    loop {
        let signal = SETTINGS_STORE_SIGNAL.wait().await;
        save_settings(&fs).await;
        if signal == SettingsStoreSignal::SaveAndReboot {
            rom_data::reboot(0, 0, 0, 0);
        }
    }
}

pub async fn load_settings(fs: &Filesystem<'_, FlashStorage<'_>>) {
    let mut buffer = [0u8; 1024 * 5];

    //fs.remove(SETTINGS_PATH).unwrap();
    // Check if file exists and create it if it doesn't
    match fs.open_file_with_options_and_then(
        |options| options.read(true).write(true).create(true),
        SETTINGS_PATH,
        |file| {
            // Check if file exists
            if file.is_empty().unwrap_or(true) {
                try_log("[STORAGE] Settings file does not exist. Creating default settings.");
                let length =
                    serde_json_core::to_slice(&StoredSettings::default(), &mut buffer).unwrap();
                try_log_debug(&length);
                match file.write(&buffer[0..length]) {
                    Ok(_) => {
                        try_log("[STORAGE] Settings file created successfully.");
                    }
                    Err(e) => {
                        try_log("[STORAGE] Failed to create settings file.");
                        try_log_debug(&e);
                    }
                }
            }
            Ok(())
        },
    ) {
        Ok(_) => {}
        Err(e) => {
            try_log("[STORAGE] Failed to check existence of settings file.");
            try_log_debug(&e);
        }
    }

    buffer = [0u8; 1024 * 5];

    // Read settings from file
    let _ = fs.open_file_with_options_and_then(
        |options| options.read(true).write(false).create(false),
        SETTINGS_PATH,
        |file| {
            try_log("[STORAGE] Reading settings from filesystem");
            let length = file.read(&mut buffer).unwrap();

            try_log_debug(&file.len().unwrap());

            match serde_json_core::from_slice::<StoredSettings>(&buffer[0..length]) {
                Ok((settings, _)) => {
                    try_log("[STORAGE] Loading settings from filesystem");
                    DMX_PORT_CONFIG
                        .try_lock()
                        .unwrap()
                        .clone_from(&settings.dmx_ports);
                    ARTNET_NODE_CONFIG
                        .try_lock()
                        .unwrap()
                        .clone_from(&settings.artnet_config);
                    NETWORK_NODE_CONFIG
                        .try_lock()
                        .unwrap()
                        .clone_from(&settings.network_config);
                }
                Err(e) => {
                    try_log("[STORAGE] Failed to load settings from filesystem");
                    try_log_debug(&e);
                    // first 100 byte of buffer to string
                    let mut buffer_vec: Vec<u8, 100> = Vec::new();
                    for i in 0..100 {
                        buffer_vec.push(buffer[i]);
                    }
                    try_log(&String::from_utf8(buffer_vec).unwrap());
                }
            }

            // TODO: Check settings version

            Ok(())
        },
    );
}

pub async fn save_settings(fs: &Filesystem<'_, FlashStorage<'_>>) {
    try_log("[STORAGE] Saving settings to filesystem");

    let mut buffer = [0u8; 1024];

    let settings = StoredSettings {
        version: SETTINGS_VERSION,
        dmx_ports: DMX_PORT_CONFIG.try_lock().unwrap().clone(),
        network_config: NETWORK_NODE_CONFIG.try_lock().unwrap().clone(),
        artnet_config: ARTNET_NODE_CONFIG.try_lock().unwrap().clone(),
    };
    serde_json_core::to_slice(&settings, &mut buffer).unwrap();
    fs.open_file_with_options_and_then(
        |options| options.read(true).write(true).create(true),
        SETTINGS_PATH,
        |file| {
            file.write(&buffer);
            Ok(())
        },
    );
    try_log("[STORAGE] Settings saved to filesystem");
}
