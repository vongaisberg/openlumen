use crate::storage::StorageError;
use embassy_rp::flash::{Async, ERASE_SIZE, FLASH_BASE, WRITE_SIZE};
use littlefs2::{driver::Storage, io::Error};

const FLASH_SIZE: usize = 4 * 1024 * 1024; // 4MB

struct FlashStorage<'a> {
    flash: embassy_rp::flash::Flash<'a, embassy_rp::peripherals::FLASH, Async, FLASH_SIZE>
}


impl<'a> Storage for FlashStorage<'a> {
    fn read(&mut self, offset: usize, data: &mut [u8]) -> Result<usize, Error> {
        self.flash.read(offset as u32, data).await.map_err(|_| Error::Io(io::Error::new(io::ErrorKind::Other, "Flash read error")))?;
        Ok(data.len())
    }

    fn write(&mut self, offset: usize, data: &[u8]) -> Result<usize, Error> {
        Ok(())
    }
    
    const READ_SIZE: usize = embassy_rp::flash::READ_SIZE;
    
    const WRITE_SIZE: usize = embassy_rp::flash::WRITE_SIZE;
    
    const BLOCK_SIZE: usize = embassy_rp::flash::ERASE_SIZE;
    
    const BLOCK_COUNT: usize = FLASH_SIZE / ERASE_SIZE;
    
    type CACHE_SIZE = typenum::consts::U256;
    
    type LOOKAHEAD_SIZE = typenum::consts::U256;
    
    fn erase(&mut self, off: usize, len: usize) -> littlefs2::io::Result<usize> {
        todo!()
    }
    
    const BLOCK_CYCLES: isize = -1;
}

fn test() {
    let mut flash = embassy_rp::flash::Flash::<_, Async, FLASH_SIZE>::new(p.FLASH, p.DMA_CH0);

    let mut storage = FlashStorage::new(flash);
}