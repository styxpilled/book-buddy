use std::fs::File;
use std::os::windows::io::FromRawHandle;

use anyhow::{Result, bail};
use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
    PIPE_TYPE_BYTE, PIPE_WAIT,
};
use windows::core::HSTRING;

pub(crate) mod association;
pub(crate) mod instance;
#[cfg(all(feature = "sim", target_os = "windows"))]
pub(crate) mod sim;

/// Makes one instance of a pipe and waits for a client.
fn accept(name: &HSTRING, access: FILE_FLAGS_AND_ATTRIBUTES, out_buffer: u32) -> Result<File> {
    // SAFETY: plain arguments, and the handle is checked below.
    let pipe = unsafe {
        CreateNamedPipeW(
            name,
            access,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            out_buffer,
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
    Ok(unsafe { File::from_raw_handle(pipe.0) })
}
