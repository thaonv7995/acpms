//! Stdout duplication utilities for separating protocol communication from logging.
//!
//! The Claude CLI SDK mode uses stdout for bidirectional protocol communication.
//! To also capture logs, we create a fresh stdout pipe:
//! - ProtocolPeer reads from original stdout (control messages)
//! - LogWriter writes to fresh pipe (log injection)
//! - Child process stdout is replaced with the fresh pipe

use anyhow::{Context, Result};
use command_group::AsyncGroupChild;
use tokio::io::AsyncWrite;

#[cfg(unix)]
use std::os::unix::io::{FromRawFd, IntoRawFd, OwnedFd};

#[cfg(windows)]
use std::os::windows::io::{FromRawHandle, IntoRawHandle, OwnedHandle};

/// Create a fresh stdout pipe for log injection
///
/// ## How it works:
/// 1. Creates OS pipe (reader, writer)
/// 2. Replaces child's stdout with pipe reader
/// 3. Returns async writer (for LogWriter to use)
///
/// ## Result:
/// - Original stdout → ProtocolPeer reads control messages
/// - New pipe → LogWriter writes logs
///
/// ## Safety:
/// Uses unsafe raw FD/handle operations. Ownership transfer ensures no double-free.
pub fn create_stdout_pipe_writer(
    child: &mut AsyncGroupChild,
) -> Result<impl AsyncWrite + Send + Unpin + 'static> {
    // Create OS pipe
    let (pipe_reader, pipe_writer) =
        os_pipe::pipe().context("Failed to create OS pipe for stdout duplication")?;

    // Replace child stdout with pipe reader
    child.inner().stdout = Some(
        wrap_fd_as_child_stdout(pipe_reader)
            .context("Failed to wrap pipe reader as ChildStdout")?,
    );

    // Return async writer
    wrap_fd_as_tokio_writer(pipe_writer).context("Failed to wrap pipe writer as tokio AsyncWrite")
}

#[cfg(unix)]
fn wrap_fd_as_child_stdout(
    pipe_reader: os_pipe::PipeReader,
) -> Result<tokio::process::ChildStdout> {
    use std::process::ChildStdout as StdChildStdout;

    // Transfer ownership: PipeReader → raw FD → OwnedFd → std::process::ChildStdout → tokio::process::ChildStdout
    let raw_fd = pipe_reader.into_raw_fd();
    let owned_fd = unsafe { OwnedFd::from_raw_fd(raw_fd) };
    let std_stdout = StdChildStdout::from(owned_fd);
    tokio::process::ChildStdout::from_std(std_stdout)
        .context("Failed to convert std::process::ChildStdout to tokio::process::ChildStdout")
}

#[cfg(unix)]
fn wrap_fd_as_tokio_writer(
    pipe_writer: os_pipe::PipeWriter,
) -> Result<impl AsyncWrite + Send + Unpin + 'static> {
    // Transfer ownership: PipeWriter → raw FD → OwnedFd → std::fs::File → tokio::fs::File
    let raw_fd = pipe_writer.into_raw_fd();
    let owned_fd = unsafe { OwnedFd::from_raw_fd(raw_fd) };
    let std_file = std::fs::File::from(owned_fd);
    Ok(tokio::fs::File::from_std(std_file))
}

#[cfg(windows)]
fn wrap_fd_as_child_stdout(
    pipe_reader: os_pipe::PipeReader,
) -> Result<tokio::process::ChildStdout> {
    use std::process::ChildStdout as StdChildStdout;

    // Transfer ownership: PipeReader → raw handle → OwnedHandle → std::process::ChildStdout → tokio::process::ChildStdout
    let raw_handle = pipe_reader.into_raw_handle();
    let owned_handle = unsafe { OwnedHandle::from_raw_handle(raw_handle) };
    let std_stdout = StdChildStdout::from(owned_handle);
    tokio::process::ChildStdout::from_std(std_stdout)
        .context("Failed to convert std::process::ChildStdout to tokio::process::ChildStdout")
}

#[cfg(windows)]
fn wrap_fd_as_tokio_writer(
    pipe_writer: os_pipe::PipeWriter,
) -> Result<impl AsyncWrite + Send + Unpin + 'static> {
    // Transfer ownership: PipeWriter → raw handle → OwnedHandle → std::fs::File → tokio::fs::File
    let raw_handle = pipe_writer.into_raw_handle();
    let owned_handle = unsafe { OwnedHandle::from_raw_handle(raw_handle) };
    let std_file = std::fs::File::from(owned_handle);
    Ok(tokio::fs::File::from_std(std_file))
}
