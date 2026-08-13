//! The control channel: a loopback unix-domain socket speaking the
//! newline-JSON [`crate::protocol`]. The daemon runs the [`Server`]; the TUI and
//! desktop use the [`Client`]. It is never a network port — only a unix socket,
//! reachable solely by local processes with filesystem access to it.
//!
//! A [`Handler`] supplies the daemon's behavior, so the server is testable with
//! a fake over an in-process socket pair. Malformed frames get a
//! [`Response::Error`] and the connection continues — a bad line never kills the
//! server.

use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::protocol::{decode_line, encode_line, DaemonStatus, Request, Response};

/// The default subscribe push interval.
const DEFAULT_SUBSCRIBE_INTERVAL: Duration = Duration::from_secs(2);

/// The daemon behavior the server dispatches to.
pub trait Handler: Send + Sync {
    /// Handles one non-subscribe request, returning the response to send.
    fn handle(&self, request: Request) -> Response;
    /// A current snapshot, used for subscribe pushes.
    fn snapshot(&self) -> DaemonStatus;
}

/// The control server: accepts connections and dispatches them to a [`Handler`].
pub struct Server {
    handler: Arc<dyn Handler>,
    subscribe_interval: Duration,
}

impl Server {
    /// Creates a server backed by `handler`.
    #[must_use]
    pub fn new(handler: Arc<dyn Handler>) -> Self {
        Self {
            handler,
            subscribe_interval: DEFAULT_SUBSCRIBE_INTERVAL,
        }
    }

    /// Overrides the interval between subscribe pushes.
    #[must_use]
    pub fn with_subscribe_interval(mut self, interval: Duration) -> Self {
        self.subscribe_interval = interval;
        self
    }

    /// Accepts connections forever, handling each on its own thread. A transient
    /// accept error is skipped rather than fatal.
    ///
    /// # Errors
    /// Currently never returns `Err`; the signature leaves room for a future
    /// fatal condition.
    pub fn serve(&self, listener: &UnixListener) -> io::Result<()> {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            let handler = Arc::clone(&self.handler);
            let interval = self.subscribe_interval;
            thread::spawn(move || {
                let _ = serve_connection(stream, handler.as_ref(), interval);
            });
        }
        Ok(())
    }
}

/// Serves one connection: reads newline-JSON requests and writes responses until
/// the peer disconnects. A `Subscribe` request switches the connection into a
/// push stream. A malformed line yields a [`Response::Error`] and the loop
/// continues.
///
/// # Errors
/// Returns an I/O error only if the socket itself fails.
pub fn serve_connection(
    stream: UnixStream,
    handler: &dyn Handler,
    subscribe_interval: Duration,
) -> io::Result<()> {
    let reader = BufReader::new(stream.try_clone()?);
    let mut writer = stream;
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        match decode_line::<Request>(&line) {
            Ok(Request::Subscribe) => {
                return stream_updates(&mut writer, handler, subscribe_interval);
            }
            Ok(request) => write_response(&mut writer, &handler.handle(request))?,
            Err(err) => write_response(
                &mut writer,
                &Response::Error {
                    message: format!("bad request: {err}"),
                },
            )?,
        }
    }
    Ok(())
}

/// Pushes an update immediately and then every `interval` until the peer closes.
fn stream_updates(
    writer: &mut impl Write,
    handler: &dyn Handler,
    interval: Duration,
) -> io::Result<()> {
    loop {
        let update = Response::Update {
            status: handler.snapshot(),
        };
        if write_response(writer, &update).is_err() {
            return Ok(()); // the client went away
        }
        thread::sleep(interval);
    }
}

/// Writes one response as a JSON line and flushes.
fn write_response(writer: &mut impl Write, response: &Response) -> io::Result<()> {
    let line = encode_line(response).map_err(io::Error::other)?;
    writer.write_all(line.as_bytes())?;
    writer.flush()
}

/// A client of the control socket.
pub struct Client {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
}

impl Client {
    /// Connects to the daemon's socket at `path`.
    ///
    /// # Errors
    /// Returns an I/O error if the socket cannot be connected.
    pub fn connect(path: &Path) -> io::Result<Self> {
        Self::from_stream(UnixStream::connect(path)?)
    }

    /// Wraps an already-connected stream (used by tests over a socket pair).
    ///
    /// # Errors
    /// Returns an I/O error if the stream cannot be cloned.
    pub fn from_stream(stream: UnixStream) -> io::Result<Self> {
        Ok(Self {
            reader: BufReader::new(stream.try_clone()?),
            writer: stream,
        })
    }

    /// Sends a request and reads exactly one response.
    ///
    /// # Errors
    /// Returns an I/O error on a socket or protocol failure.
    pub fn request(&mut self, request: &Request) -> io::Result<Response> {
        self.send(request)?;
        self.read_response()
    }

    /// Sends a request without waiting for a response (used before streaming
    /// reads, e.g. after `Subscribe`).
    ///
    /// # Errors
    /// Returns an I/O error if the request cannot be written.
    pub fn send(&mut self, request: &Request) -> io::Result<()> {
        let line = encode_line(request).map_err(io::Error::other)?;
        self.writer.write_all(line.as_bytes())?;
        self.writer.flush()
    }

    /// Reads one response line from the daemon.
    ///
    /// # Errors
    /// Returns an I/O error on EOF, a socket failure, or a malformed line.
    pub fn read_response(&mut self) -> io::Result<Response> {
        let mut line = String::new();
        if self.reader.read_line(&mut line)? == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "control connection closed",
            ));
        }
        decode_line(&line).map_err(io::Error::other)
    }
}

/// The default control-socket path: `$XDG_RUNTIME_DIR/beagle-agentd.sock`, else
/// `~/.local/state/beagle/agentd.sock`, else a relative fallback.
#[must_use]
pub fn default_socket_path() -> PathBuf {
    if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir).join("beagle-agentd.sock");
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".local/state/beagle/agentd.sock");
    }
    PathBuf::from("beagle-agentd.sock")
}

/// Binds a listener at `path`, creating the parent directory and removing any
/// stale socket left by a previous run.
///
/// # Errors
/// Returns an I/O error if the directory, cleanup, or bind fails.
pub fn bind(path: &Path) -> io::Result<UnixListener> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => return Err(err),
    }
    UnixListener::bind(path)
}

#[cfg(test)]
#[path = "tests/control.rs"]
mod tests;
