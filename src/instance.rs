//! One running app for each user. A later start sends its book path to the
//! running app through a named pipe, and then exits.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::windows::io::FromRawHandle;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use futures::channel::mpsc;
use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
use windows::Win32::Storage::FileSystem::PIPE_ACCESS_INBOUND;
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
    PIPE_TYPE_BYTE, PIPE_WAIT,
};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::{ASFW_ANY, AllowSetForegroundWindow};
use windows::core::HSTRING;

/// The longest time that a later start tries to reach the running app.
const SEND_TIMEOUT: Duration = Duration::from_secs(3);

/// A path from a later start. `None` only brings the window to the front.
pub type Opened = mpsc::UnboundedReceiver<Option<PathBuf>>;

/// For the first start, the paths of later starts. A later start sends its
/// path and gets `None`.
pub fn claim(path: Option<&Path>) -> Option<Opened> {
    let mutex = HSTRING::from(format!(r"Local\{}", name()));
    // SAFETY: plain arguments. The handle stays open until the process exits.
    let created = unsafe { CreateMutexW(None, false, &mutex) };
    // SAFETY: no arguments.
    let exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    if created.is_ok() && exists {
        match send(path) {
            Ok(()) => return None,
            Err(e) => log::error!("cannot send the path to the running app: {e:#}"),
        }
    }
    let (sender, receiver) = mpsc::unbounded();
    std::thread::spawn(move || {
        if let Err(e) = listen(sender) {
            log::error!("instance pipe: {e:#}");
        }
    });
    Some(receiver)
}

/// The dev build has its own names, so it never sends a book to the release
/// build. A pipe name is the same for all users, so it has the user name.
fn name() -> String {
    let app = if cfg!(debug_assertions) {
        "book-buddy-dev"
    } else {
        "book-buddy"
    };
    let user = std::env::var("USERNAME").unwrap_or_default();
    format!("{app}-{user}")
}

fn pipe() -> String {
    format!(r"\\.\pipe\{}", name())
}

fn send(path: Option<&Path>) -> Result<()> {
    let deadline = Instant::now() + SEND_TIMEOUT;
    // The pipe is busy or not made yet while the running app reads another start.
    let mut file = loop {
        match OpenOptions::new().write(true).open(pipe()) {
            Ok(file) => break file,
            Err(_) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(e.into()),
        }
    };
    // So that the running app can bring its window to the front.
    // SAFETY: plain arguments.
    unsafe { AllowSetForegroundWindow(ASFW_ANY) }.ok();
    let line = path.map(|path| path.to_string_lossy().into_owned());
    writeln!(file, "{}", line.unwrap_or_default())?;
    Ok(())
}

fn listen(sender: mpsc::UnboundedSender<Option<PathBuf>>) -> Result<()> {
    let name = HSTRING::from(pipe());
    loop {
        // SAFETY: plain arguments, and the handle is checked below.
        let pipe = unsafe {
            CreateNamedPipeW(
                &name,
                PIPE_ACCESS_INBOUND,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                0,
                4096,
                0,
                None,
            )
        };
        if pipe.is_invalid() {
            bail!("CreateNamedPipeW: {}", windows::core::Error::from_thread());
        }
        // A client that connects before this call is not an error.
        // SAFETY: the pipe handle is valid.
        unsafe { ConnectNamedPipe(pipe, None) }.ok();
        // SAFETY: the handle is valid, and the file owns and closes it.
        let file = unsafe { File::from_raw_handle(pipe.0) };
        let mut line = String::new();
        BufReader::new(file).read_line(&mut line).ok();
        let line = line.trim();
        sender.unbounded_send((!line.is_empty()).then(|| PathBuf::from(line)))?;
    }
}
