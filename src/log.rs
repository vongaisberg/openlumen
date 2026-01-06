use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::Instant;
use heapless::{String, Vec};
use picoserve::{request::Request, response::{IntoResponse, Response}};


const LOG_CAPACITY: usize = 100;

pub static LOG_RING: Mutex<ThreadModeRawMutex, LogRingBuffer> = Mutex::new(LogRingBuffer::new());

struct LogEntry {
    time: Instant,
    message: String<100>,
}

pub struct LogRingBuffer {
    buffer: [LogEntry; LOG_CAPACITY],
    start: usize,
    count: usize,
}

impl LogRingBuffer {
    pub const fn new() -> Self {
        const EMPTY: LogEntry = LogEntry { time: Instant::MIN, message: String::new() };
        Self {
            buffer: [EMPTY; LOG_CAPACITY],
            start: 0,
            count: 0,
        }
    }

    pub fn push(&mut self, entry: LogEntry) {
        let end = (self.start + self.count) % LOG_CAPACITY;

        self.buffer[end] = entry;
        if self.count < LOG_CAPACITY {
            self.count += 1;
        } else {
            // full: advance start to overwrite oldest
            self.start = (self.start + 1) % LOG_CAPACITY;
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &LogEntry> {
        (0..self.count).map(move |i| {
            let idx = (self.start + i) % LOG_CAPACITY;
            &self.buffer[idx]
        })
    }
} 

pub use core::fmt::{Debug, Write};


pub async fn log(message: &str) {
    let time = Instant::now();
    let mut s: String<100> = String::new();
    let _ = s.push_str(message);
    LOG_RING.lock().await.push(LogEntry { time, message: s });
}
/// Logs a value that implements the Debug trait.
/// 
/// Usage: `log_debug(&some_value).await;`
async fn log_debug<T: Debug>(value: &T) {
    let time = Instant::now();
    let mut s: String<100> = String::new();
    // Use core::fmt::Write to format Debug into the string
    let _ = core::write!(&mut s, "{:?}", value);
    LOG_RING.lock().await.push(LogEntry { time, message: s });
}

pub fn try_log(message: &str) {
    let time = Instant::now();
    let mut s: String<100> = String::new();
    let _ = s.push_str(message);
    if let Ok(mut guard) = LOG_RING.try_lock() {
        guard.push(LogEntry { time, message: s });
    }
}



fn try_log_debug<T: Debug>(value: &T) {
    let time = Instant::now();
    if let Ok(mut guard) = LOG_RING.try_lock() {
        let mut s: String<100> = String::new();
        let _ = core::write!(&mut s, "{:?}", value);
        guard.push(LogEntry { time, message: s });
    }

}

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        async {
            use core::fmt::Write;
            let mut s: heapless::String<100> = heapless::String::new();
            let _ = core::write!(&mut s, $($arg)*);
            $crate::log::log(s.as_str()).await
        }
    };
}

#[macro_export]
macro_rules! try_log {
    ($($arg:tt)*) => {
        
            let mut s: heapless::String<100> = heapless::String::new();
            let _ = core::write!(&mut s, $($arg)*);
            $crate::log::try_log(s.as_str())
    };
}


pub async fn get_logs() -> String<3000> {
    let buffer = LOG_RING.lock().await;
    let mut logs = String::new();
    for entry in buffer.iter() {
        let _ = core::write!(&mut logs, "{:>10}", entry.time.as_micros());
        let _ = core::write!(&mut logs, " ");
        let _ = logs.push_str(entry.message.as_str());
        let _ = logs.push_str("\n");
    }
    logs
}

pub async fn serve_logs() -> impl IntoResponse {
    get_logs().await
}