use crate::{Error, Result};
use pipe::{PipeBufWriter, PipeReader};
use std::fmt::{self, Debug};
use std::io::{Error as IoError, ErrorKind, Read, Result as IoResult, Write};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::Mutex;
use std::thread::spawn;

pub struct HttpWriter {
    write: PipeBufWriter,
    rx: Mutex<Receiver<Result<()>>>,
}

/// A wrapper for the read end of the pipe that sniches on when data is first read
/// by sending `Ok(())` down tx.
///
/// This is used so that we can block the code making the put request until ethier:
/// a) the data is tried to be read, or
/// b) the request fails before trying to send the payload (bad hostname, invalid auth, etc)
struct SnitchingReader {
    read: PipeReader,
    connected: bool,
    tx: SyncSender<Result<()>>,
}

impl Read for SnitchingReader {
    fn read(&mut self, buffer: &mut [u8]) -> IoResult<usize> {
        if !self.connected {
            self.tx
                .send(Ok(()))
                .map_err(|e| IoError::new(ErrorKind::Other, e))?;
            self.connected = true;
        }
        self.read.read(buffer)
    }
}

impl HttpWriter {
    pub fn new(url: &str, size: Option<u64>) -> Result<Self> {
        let (read, write) = pipe::pipe_buffered();

        let mut req = ureq::put(url);
        if let Some(size) = size {
            req = req.set("content-length", &size.to_string());
        }

        let (done_tx, rx) = sync_channel(0);
        let snitch = SnitchingReader {
            read,
            connected: false,
            tx: done_tx.clone(),
        };

        spawn(move || {
            done_tx
                .send(req.send(snitch).map(|_| ()).map_err(|e| e.into()))
                .unwrap();
        });

        // either Ok(()) if the other thread started reading or the connection error
        rx.recv().unwrap()?;
        let rx = Mutex::new(rx);
        Ok(HttpWriter { write, rx })
    }

    pub fn finish(self) -> Result<()> {
        drop(self.write);
        self.rx
            .try_lock()
            .expect("clio HttpWriter lock should one ever be taken once while dropping")
            .recv()
            .unwrap()?;
        Ok(())
    }
}

impl Write for HttpWriter {
    fn write(&mut self, buffer: &[u8]) -> IoResult<usize> {
        self.write.write(buffer)
    }
    fn flush(&mut self) -> IoResult<()> {
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
    #[cfg(feature = "clap-parse")]
    read: Mutex<Box<dyn Read + Send>>,
    #[cfg(not(feature = "clap-parse"))]
    read: Box<dyn Read + Send>,
}

impl HttpReader {
    pub fn new(url: &str) -> Result<Self> {
        let resp = ureq::get(url).call()?;

        let length = resp
            .header("content-length")
            .and_then(|x| x.parse::<u64>().ok());
        Ok(HttpReader {
            length,
            #[cfg(not(feature = "clap-parse"))]
            read: Box::new(resp.into_reader()),
            #[cfg(feature = "clap-parse")]
            read: Mutex::new(Box::new(resp.into_reader())),
        })
    }

    pub fn len(&self) -> Option<u64> {
        self.length
    }
}

impl Read for HttpReader {
    #[cfg(not(feature = "clap-parse"))]
    fn read(&mut self, buffer: &mut [u8]) -> IoResult<usize> {
        self.read.read(buffer)
    }

    #[cfg(feature = "clap-parse")]
    fn read(&mut self, buffer: &mut [u8]) -> IoResult<usize> {
        self.read
            .lock()
            .map_err(|_| IoError::new(ErrorKind::Other, "Error locking HTTP reader"))?
            .read(buffer)
    }
}

impl Debug for HttpReader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpReader").finish()
    }
}

impl From<ureq::Error> for Error {
    fn from(err: ureq::Error) -> Self {
        match err {
            ureq::Error::Status(code, resp) => Error::Http {
                code,
                message: resp.status_text().to_owned(),
            },
            _ => Error::Http {
                code: 499,
                message: err.to_string(),
            },
        }
    }
}
