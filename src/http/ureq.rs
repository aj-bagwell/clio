use crate::{Error, Result};
use pipe::{PipeReader, PipeWriter};
use std::fmt::{self, Debug};
use std::io::{Error as IoError, ErrorKind, Read, Result as IoResult, Write};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::thread::spawn;

pub struct HttpWriter {
    write: PipeWriter,
    rx: Receiver<Result<()>>,
}

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
        let (read, write) = pipe::pipe();

        let mut req = ureq::put(&url);
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

        rx.recv().unwrap()?;
        Ok(HttpWriter { write, rx })
    }

    pub fn finish(self) -> Result<()> {
        drop(self.write);
        self.rx.recv().unwrap()?;
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
    read: Box<dyn Read + Send>,
}

impl HttpReader {
    pub fn new(url: &str) -> Result<Self> {
        let resp = ureq::get(&url).call()?;

        let length = resp
            .header("content-length")
            .and_then(|x| x.parse::<u64>().ok());
        Ok(HttpReader {
            length,
            read: Box::new(resp.into_reader()),
        })
    }

    pub fn len(&self) -> Option<u64> {
        self.length
    }
}

impl Read for HttpReader {
    fn read(&mut self, buffer: &mut [u8]) -> IoResult<usize> {
        self.read.read(buffer)
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
