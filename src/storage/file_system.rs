use embassy_rp::{rom_data, Peri};
use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, signal::Signal};
use core::fmt::Write;

use heapless::{String, Vec};
use littlefs2::{
    fs::{Filesystem},
    path,
};
use littlefs2_core::Path;

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
    
    let free_space = fs.available_space().unwrap_or(0);
    try_log!(
        "[FLASH] Filesystem mounted successfully. Free space: {}",
        free_space
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

/// Result of loading settings from flash
enum LoadResult {
    /// Settings loaded successfully
    Success,
    /// Settings file was empty or missing - created defaults
    CreatedDefaults,
    /// Settings had wrong version - needs reset
    VersionMismatch,
    /// Settings failed to parse - needs reset
    ParseError,
    /// I/O error reading settings
    IoError,
}

pub async fn load_settings(fs: &Filesystem<'_, FlashStorage<'_>>) {
    let mut buffer = [0u8; 1024 * 5];
    let mut load_result = LoadResult::Success;

    // Step 1: Try to read and parse existing settings
    let read_result = fs.open_file_with_options_and_then(
        |options| options.read(true).write(false).create(false),
        SETTINGS_PATH,
        |file| {
            let length = match file.len() {
                Ok(len) => len,
                Err(e) => {
                    try_log!("[FLASH] Failed to get file length: {:?}", e);
                    load_result = LoadResult::IoError;
                    return Ok(());
                }
            };

            if length == 0 {
                try_log!("[FLASH] Settings file is empty, will create defaults");
                load_result = LoadResult::CreatedDefaults;
                return Ok(());
            }

            try_log!("[FLASH] Reading settings from filesystem. Length: {}", length);

            if let Err(e) = file.read(&mut buffer[0..length]) {
                try_log!("[FLASH] Failed to read settings file: {:?}", e);
                load_result = LoadResult::IoError;
                return Ok(());
            }

            log_until_null(&buffer[0..length]);

            match serde_json_core::from_slice::<StoredSettings>(&buffer[0..length]) {
                Ok((settings, _)) => {
                    // Check version before applying settings
                    if settings.version != SETTINGS_VERSION {
                        try_log!(
                            "[FLASH] Settings version mismatch: stored={}, expected={}. Resetting to defaults.",
                            settings.version,
                            SETTINGS_VERSION
                        );
                        load_result = LoadResult::VersionMismatch;
                        return Ok(());
                    }

                    // Apply settings to runtime configs
                    if let Ok(mut dmx_config) = DMX_PORT_CONFIG.try_lock() {
                        dmx_config.clone_from(&settings.dmx_ports);
                    } else {
                        try_log!("[FLASH] Warning: Could not lock DMX_PORT_CONFIG");
                    }

                    if let Ok(mut artnet_config) = ARTNET_NODE_CONFIG.try_lock() {
                        artnet_config.clone_from(&settings.artnet_config);
                    } else {
                        try_log!("[FLASH] Warning: Could not lock ARTNET_NODE_CONFIG");
                    }

                    if let Ok(mut network_config) = NETWORK_NODE_CONFIG.try_lock() {
                        network_config.clone_from(&settings.network_config);
                    } else {
                        try_log!("[FLASH] Warning: Could not lock NETWORK_NODE_CONFIG");
                    }

                    try_log!("[FLASH] Settings version {} loaded successfully", settings.version);
                    load_result = LoadResult::Success;
                }
                Err(e) => {
                    try_log!(
                        "[FLASH] Failed to parse settings: {:?}. Resetting to defaults.",
                        e
                    );
                    log_until_null(&buffer[0..length]);
                    load_result = LoadResult::ParseError;
                }
            }

            Ok(())
        },
    );

    // If file doesn't exist, mark as needing defaults
    if read_result.is_err() {
        try_log!("[FLASH] Settings file not found, creating defaults");
        load_result = LoadResult::CreatedDefaults;
    }

    // Step 2: If we need to reset/create settings, do so now
    match load_result {
        LoadResult::Success => {
            // Settings loaded successfully, nothing more to do
        }
        LoadResult::CreatedDefaults | LoadResult::VersionMismatch | LoadResult::ParseError | LoadResult::IoError => {
            try_log!("[FLASH] Creating default settings file");
            
            // Remove existing file if present (ignore errors - file might not exist)
            let _ = fs.remove(SETTINGS_PATH);

            // Serialize default settings
            let default_settings = StoredSettings::default();
            let length = match serde_json_core::to_slice(&default_settings, &mut buffer) {
                Ok(len) => len,
                Err(e) => {
                    try_log!("[FLASH] Failed to serialize default settings: {:?}", e);
                    return;
                }
            };

            // Write default settings to file
            let write_result = fs.open_file_with_options_and_then(
                |options| options.read(false).write(true).create(true).truncate(true),
                SETTINGS_PATH,
                |file| {
                    match file.write(&buffer[0..length]) {
                        Ok(_) => {
                            try_log!("[FLASH] Default settings file created successfully");
                        }
                        Err(e) => {
                            try_log!("[FLASH] Failed to write default settings: {:?}", e);
                        }
                    }
                    Ok(())
                },
            );

            if let Err(e) = write_result {
                try_log!("[FLASH] Failed to create settings file: {:?}", e);
            }

            // Runtime configs already have defaults from their static initializers,
            // so we don't need to explicitly apply them
        }
    }
}

async fn save_settings(fs: &Filesystem<'_, FlashStorage<'_>>) {
    try_log!("[FLASH] Saving settings to filesystem");

    let mut buffer = [0u8; 1024];

    // Collect settings from runtime configs with error handling
    let dmx_ports = match DMX_PORT_CONFIG.try_lock() {
        Ok(config) => config.clone(),
        Err(_) => {
            try_log!("[FLASH] Warning: Could not lock DMX_PORT_CONFIG for save, using defaults");
            core::array::from_fn(crate::schema::DmxPortConfig::default_with_universe)
        }
    };

    let network_config = match NETWORK_NODE_CONFIG.try_lock() {
        Ok(config) => config.clone(),
        Err(_) => {
            try_log!("[FLASH] Warning: Could not lock NETWORK_NODE_CONFIG for save, using defaults");
            crate::schema::NetworkConfig::default()
        }
    };

    let artnet_config = match ARTNET_NODE_CONFIG.try_lock() {
        Ok(config) => config.clone(),
        Err(_) => {
            try_log!("[FLASH] Warning: Could not lock ARTNET_NODE_CONFIG for save, using defaults");
            crate::schema::ArtnetConfig::default()
        }
    };

    let settings = StoredSettings {
        version: SETTINGS_VERSION,
        dmx_ports,
        network_config,
        artnet_config,
    };

    let length = match serde_json_core::to_slice(&settings, &mut buffer) {
        Ok(len) => len,
        Err(e) => {
            try_log!("[FLASH] Failed to serialize settings: {:?}", e);
            return;
        }
    };

    // Remove existing file - ignore errors (file might not exist)
    if let Err(e) = fs.remove(SETTINGS_PATH) {
        // Only log if it's not a "not found" error
        try_log!("[FLASH] Note: Could not remove old settings file: {:?}", e);
    }

    // Write new settings file
    match fs.open_file_with_options_and_then(
        |options| options.read(false).write(true).create(true).truncate(true),
        SETTINGS_PATH,
        |file| {
            match file.write(&buffer[0..length]) {
                Ok(_) => {
                    try_log!("[FLASH] Settings saved to filesystem successfully");
                }
                Err(e) => {
                    try_log!("[FLASH] Failed to write settings to filesystem: {:?}", e);
                }
            }
            Ok(())
        },
    ) {
        Ok(_) => {}
        Err(e) => {
            try_log!("[FLASH] Failed to open settings file for writing: {:?}", e);
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