use curl::easy::{Easy, ReadError};
use curl::Error;
use pipe::{PipeBufWriter, PipeReader};
use std::convert::TryFrom;
use std::fmt::{self, Debug};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver};
use std::sync::{Arc, Mutex};
use std::thread::spawn;

pub struct HttpWriter {
    write: PipeBufWriter,
    rx: Mutex<Receiver<Result<(), Error>>>,
}

impl HttpWriter {
    pub fn new(url: &str, size: Option<u64>) -> Result<Self, Error> {
        let mut easy = new_easy(url)?;

        let (mut read, write) = pipe::pipe_buffered();

        let (done_tx, rx) = sync_channel(0);
        let connected_tx = done_tx.clone();

        let mut connected = false;

        easy.put(true)?;
        easy.upload(true)?;
        if let Some(size) = size {
            easy.in_filesize(size)?;
        }
        easy.read_function(move |into| {
            if !connected {
                connected_tx.send(Ok(())).map_err(|_| ReadError::Abort)?;
                connected = true;
            }
            let len = read.read(into).unwrap();
            eprintln!("read: {}", len);
            Ok(len)
        })?;
        spawn(move || {
            done_tx.send(easy.perform()).unwrap();
        });

        rx.recv().unwrap()?;
        let rx = Mutex::new(rx);

        Ok(HttpWriter { write, rx })
    }

    pub fn finish(self) -> Result<(), Error> {
        drop(self.write);
        self.rx
            .try_lock()
            .expect("clio HttpReader lock should one ever be taken once while dropping")
            .recv()
            .unwrap()?;
        Ok(())
    }
}

impl Write for HttpWriter {
    fn write(&mut self, buffer: &[u8]) -> Result<usize, std::io::Error> {
        self.write.write(buffer)
    }
    fn flush(&mut self) -> Result<(), std::io::Error> {
        self.write.flush()
    }
}

impl fmt::Debug for HttpWriter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpWriter").finish()
    }
}

pub struct HttpReader {
    length: Option<u64>,
    read: PipeReader,
    rx: Mutex<Receiver<Result<(), Error>>>,
}

impl HttpReader {
    pub fn new(url: &str) -> Result<Self, Error> {
        let url = url.to_owned();

        let (read, mut write) = pipe::pipe();

        let (done_tx, rx) = sync_channel(0);
        let connected_tx = done_tx.clone();

        let mut connected = false;
        let length = Arc::new(AtomicI64::new(-1));

        let mut easy = new_easy(&url)?;
        easy.header_function({
            let length = length.clone();
            move |data| {
                let data = std::str::from_utf8(data).unwrap().to_lowercase();
                if let Some(length_string) = data.strip_prefix("content-length:") {
                    length.store(
                        length_string.trim().parse::<i64>().unwrap_or(-1),
                        Ordering::Relaxed,
                    );
                }
                if data.starts_with("http/") {
                    length.store(-1, Ordering::Relaxed);
                }
                true
            }
        })?;

        easy.write_function({
            let length = length.clone();
            move |data| {
                if !connected {
                    if data.is_empty() {
                        length.store(-1, Ordering::Relaxed);
                    }
                    if connected_tx.send(Ok(())).is_err() {
                        // if the message queue is broken return 0 to curl to indicate a problem
                        return Ok(0);
                    }
                    connected = true;
                }

                if write.write_all(data).is_err() {
                    // if the pipe is broken return 0 to curl to indicate a problem
                    return Ok(0);
                }
                Ok(data.len())
            }
        })?;

        spawn(move || {
            let err = easy.perform();
            drop(easy);
            done_tx.send(err).unwrap();
        });

        rx.recv().unwrap()?;
        let rx = Mutex::new(rx);

        let length = u64::try_from(length.load(Ordering::Relaxed)).ok();
        Ok(HttpReader { length, read, rx })
    }

    pub fn len(&self) -> Option<u64> {
        self.length
    }

    #[allow(dead_code)]
    pub fn finish(self) -> Result<(), Error> {
        drop(self.read);
        self.rx
            .try_lock()
            .expect("clio HttpWriter lock should one ever be taken once while dropping")
            .recv()
            .unwrap()?;
        Ok(())
    }
}

impl Read for HttpReader {
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, std::io::Error> {
        self.read.read(buffer)
    }
}

impl Debug for HttpReader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpReader").finish()
    }
}

fn new_easy(url: &str) -> Result<Easy, Error> {
    let mut easy = Easy::new();
    easy.url(url)?;
    easy.follow_location(true)?;
    easy.fail_on_error(true)?;

    Ok(easy)
}

#[cfg(feature = "http")]
impl From<Error> for crate::Error {
    fn from(err: Error) -> Self {
        crate::Error::Http {
            code: 499,
            message: err.description().to_owned(),
        }
    }
}
