//! LittleFS2 filesystem storage for RP2350 flash
//!
//! This module provides a filesystem interface using LittleFS2 on a reserved
//! region of flash memory. The filesystem is placed after the application code
//! and before the settings storage area.
//!
//! Flash layout:
//! - 0x10000000: Application code (starts here)
//! - 0x10100000: Filesystem region (1MB offset, ~1008KB available)
//! - 0x101FC000: Settings storage (last 16KB of 2MB flash)
//! - 0x10200000: End of 2MB flash

use embassy_rp::flash::{Async, ERASE_SIZE, WRITE_SIZE};
use littlefs2::{driver::Storage, io::Error};

/// Total flash size (matches memory.x)
const FLASH_SIZE: usize = 2 * 1024 * 1024; // 2MB

/// Base offset for filesystem region
/// Filesystem is placed at 2MB - 32KB to avoid the very last sector
/// Must be aligned to ERASE_SIZE for proper flash operations
const FILESYSTEM_BASE_OFFSET: usize = (2 * 1024 * 1024) - (16 * 1024); // 2MB - 16KB

// Compile-time check that FILESYSTEM_BASE_OFFSET is aligned to ERASE_SIZE
const _: () = {
    assert!(FILESYSTEM_BASE_OFFSET % ERASE_SIZE == 0, "FILESYSTEM_BASE_OFFSET must be aligned to ERASE_SIZE");
};

/// Available filesystem size
/// Filesystem occupies 16KB
/// Last 16KB (2MB-16KB to 2MB) is reserved for direct settings storage
const FILESYSTEM_SIZE: usize = 16 * 1024; // 16KB

/// Flash storage driver for LittleFS2
/// Wraps embassy_rp flash with offset translation for filesystem region
pub struct FlashStorage<'a> {
    flash: embassy_rp::flash::Flash<'a, embassy_rp::peripherals::FLASH, Async, FLASH_SIZE>
}

impl<'a> FlashStorage<'a> {
    /// Create a new FlashStorage instance
    pub fn new(flash: embassy_rp::flash::Flash<'a, embassy_rp::peripherals::FLASH, Async, FLASH_SIZE>) -> Self {
        Self { flash }
    }
}


impl<'a> Storage for FlashStorage<'a> {
    fn read(&mut self, offset: usize, data: &mut [u8]) -> Result<usize, Error> {
        // Add filesystem base offset to convert filesystem-relative offset to absolute flash offset
        let absolute_offset = FILESYSTEM_BASE_OFFSET + offset;

        
        self.flash.blocking_read(absolute_offset as u32, data)
            .map_err(|_| Error::IO)?;
        Ok(data.len())
    }

    fn write(&mut self, offset: usize, data: &[u8]) -> Result<usize, Error> {
        // Add filesystem base offset to convert filesystem-relative offset to absolute flash offset
        let absolute_offset = FILESYSTEM_BASE_OFFSET + offset;
        
        
        // Flash writes must be aligned to WRITE_SIZE and be a multiple of WRITE_SIZE
        // LittleFS2 should provide aligned writes based on WRITE_SIZE constant
        let write_align = absolute_offset % WRITE_SIZE;
        let data_remainder = data.len() % WRITE_SIZE;
        
        if write_align != 0 || data_remainder != 0 {
            return Err(Error::IO);
        }
        
        self.flash.blocking_write(absolute_offset as u32, data)
            .map_err(|_| Error::IO)?;
        Ok(data.len())
    }
    
    const READ_SIZE: usize = embassy_rp::flash::READ_SIZE;
    
    const WRITE_SIZE: usize = embassy_rp::flash::WRITE_SIZE;
    
    const BLOCK_SIZE: usize = embassy_rp::flash::ERASE_SIZE;
    
    /// Block count based on available filesystem size (not total flash size)
    const BLOCK_COUNT: usize = FILESYSTEM_SIZE / ERASE_SIZE;
    
    type CACHE_SIZE = typenum::consts::U256;
    
    type LOOKAHEAD_SIZE = typenum::consts::U256;
    
    fn erase(&mut self, off: usize, len: usize) -> littlefs2::io::Result<usize> {
        // Add filesystem base offset to convert filesystem-relative offset to absolute flash offset
        let absolute_offset = FILESYSTEM_BASE_OFFSET + off;

        // Perform the erase operation
        // Note: blocking_erase takes (start_offset, end_offset), not (offset, length)
        self.flash.blocking_erase(absolute_offset as u32, (absolute_offset + len) as u32)
            .map_err(|_| Error::IO)?;
        
        // Return the original length, not the aligned length
        // LittleFS2 expects the amount it requested to be erased
        Ok(len)
    }
    
    const BLOCK_CYCLES: isize = -1;
}
