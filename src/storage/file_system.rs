use embassy_rp::{gpio::Output, rom_data, Peri};
use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, signal::Signal};

use core::fmt::Write;
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
    schema::{StoredSettings, SETTINGS_VERSION},
    storage::flash_storage_adapter::FlashStorage,
    try_log,
    web_task::NETWORK_NODE_CONFIG,
};

use crate::log;
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
    log!("[FLASH] File system task starting").await;
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
                try_log!("[FLASH] Failed to format filesystem, error: {:?}", e);
                return; // Exit if format fails
            }

            // Try mounting again after format
            match Filesystem::mount(&mut alloc, &mut storage) {
                Ok(fs) => fs,
                Err(e) => {
                    try_log!("[FLASH] Failed to mount filesystem, error: {:?}", e);
                    return; // Exit if mount fails after format
                }
            }
        }
    };
    try_log!(
        "[FLASH] Filesystem mounted successfully. Free space: {}",
        fs.available_space().unwrap()
    );

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
                let length =
                    serde_json_core::to_slice(&StoredSettings::default(), &mut buffer).unwrap();
                try_log!(
                    "[FLASH] Settings file does not exist. Creating default settings. Length: {}",
                    length
                );
                let mut buffer_vec: Vec<u8, 100> = Vec::new();
                for i in 0..100 {
                    buffer_vec.push(buffer[i]);
                }
                try_log!("{}", &String::from_utf8(buffer_vec).unwrap());
                match file.write(&buffer[0..length]) {
                    Ok(_) => {
                        try_log!("[FLASH] Settings file created successfully.");
                    }
                    Err(e) => {
                        try_log!("[FLASH] Failed to create settings file. Error: {:?}", e);
                    }
                }
            }
            Ok(())
        },
    ) {
        Ok(_) => {}
        Err(e) => {
            try_log!(
                "[FLASH] Failed to check existence of settings file. Error: {:?}",
                e
            );
        }
    }

    buffer = [0u8; 1024 * 5];

    // Read settings from file
    let _ = fs.open_file_with_options_and_then(
        |options| options.read(true).write(false).create(false),
        SETTINGS_PATH,
        |file| {
            let length = file.len().unwrap();
            try_log!(
                "[FLASH] Reading settings from filesystem. Length: {}",
                length
            );

            file.read(&mut buffer[0..length]).unwrap();
            log_until_null(&buffer[0..length]);

            match serde_json_core::from_slice::<StoredSettings>(&buffer[0..length]) {
                Ok((settings, _)) => {
                    
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
                    try_log!("[FLASH] Settings version {} loaded from filesystem", settings.version);
                }
                Err(e) => {
                    try_log!(
                        "[FLASH] Failed to load settings from filesystem, error: {:?}",
                        e
                    );
                    log_until_null(&buffer[0..length]);
                }
            }

            // TODO: Check settings version

            Ok(())
        },
    );
}

async fn save_settings(fs: &Filesystem<'_, FlashStorage<'_>>) {
    try_log!("[FLASH] Saving settings to filesystem");

    let mut buffer = [0u8; 1024];

    let settings = StoredSettings {
        version: SETTINGS_VERSION,
        dmx_ports: DMX_PORT_CONFIG.try_lock().unwrap().clone(),
        network_config: NETWORK_NODE_CONFIG.try_lock().unwrap().clone(),
        artnet_config: ARTNET_NODE_CONFIG.try_lock().unwrap().clone(),
    };
    let length = serde_json_core::to_slice(&settings, &mut buffer).unwrap();

    fs.remove(SETTINGS_PATH).unwrap();

    match fs.open_file_with_options_and_then(
        |options| options.read(true).write(true).create(true),
        SETTINGS_PATH,
        |file| {
            match file.write(&buffer[0..length]) {
                Ok(_) => {
                    try_log!("[FLASH] Settings saved to filesystem");
                }
                Err(e) => {
                    try_log!("[FLASH] Failed to save settings to filesystem, error: {:?}", e);
                }
            }
            Ok(())
        },
    ) {
        Ok(_) => {
            
        }
        Err(e) => {
            try_log!("[FLASH] Failed to save settings to filesystem, error: {:?}", e);
        }
    }
}

pub fn save_config() {
    SETTINGS_STORE_SIGNAL.signal(SettingsStoreSignal::Save);
}

pub fn save_and_reboot() {
    SETTINGS_STORE_SIGNAL.signal(SettingsStoreSignal::SaveAndReboot);
}

fn log_until_null(buffer: &[u8]) {
    let mut buffer_vec: Vec<u8, 100> = Vec::new();
    for i in 0..buffer.len() {
        if buffer[i] == 0 {
            break;
        }
        buffer_vec.push(buffer[i]);
        if buffer_vec.len() >= 100 {
            try_log!("{}", &String::from_utf8(buffer_vec.clone()).unwrap());
            buffer_vec.clear();
        }
    }
    try_log!("{}", &String::from_utf8(buffer_vec).unwrap());
}