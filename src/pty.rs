use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::io::{self, Read, Write};

/// A pseudo-terminal with an attached child process.
///
/// Reads from the child are non-blocking; writes are blocking.
pub struct Pty {
    master: Box<dyn portable_pty::MasterPty + Send>,
    reader: Box<dyn Read + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
}

impl Pty {
    /// Spawn a child process connected to a new PTY of the given size.
    ///
    /// `command[0]` is the program; `command[1..]` are arguments.
    /// The master fd is set to `O_NONBLOCK` so reads never block.
    pub fn spawn(rows: u16, cols: u16, command: &[String]) -> anyhow::Result<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })?;

        let mut cmd = CommandBuilder::new(&command[0]);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("TERM_PROGRAM", "deskitty");
        if command.len() > 1 {
            cmd.args(&command[1..]);
        }

        let child = pair.slave.spawn_command(cmd)?;

        let actual_size = pair.master.get_size()?;
        if actual_size.rows != rows || actual_size.cols != cols {
            pair.master.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })?;
        }

        let raw_fd = pair.master.as_raw_fd();
        if let Some(fd) = raw_fd {
            let flags = unsafe { libc::fcntl(fd, libc::F_GETFL, 0) };
            unsafe {
                libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
            }
        }

        let reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;

        Ok(Self { master: pair.master, reader, writer, child })
    }

    /// Non-blocking read from the child process output.
    ///
    /// Returns `None` when no data is available or the child has closed its output.
    pub fn try_read(&mut self, buf: &mut [u8]) -> io::Result<Option<usize>> {
        match self.reader.read(buf) {
            Ok(0) => Ok(None),
            Ok(n) => Ok(Some(n)),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Write data to the child process stdin (blocking).
    pub fn write(&mut self, data: &[u8]) -> io::Result<()> {
        self.writer.write_all(data)
    }

    /// Resize the PTY window, sending `SIGWINCH` to the child.
    pub fn resize(&self, rows: u16, cols: u16) -> anyhow::Result<()> {
        self.master.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })?;
        Ok(())
    }

    /// Non-blocking poll for child process exit.
    ///
    /// Returns `None` if the child is still running.
    pub fn try_wait(&mut self) -> io::Result<Option<portable_pty::ExitStatus>> {
        self.child.try_wait()
    }

    /// Force-kill the child process.
    #[allow(dead_code)]
    pub fn kill(&mut self) -> io::Result<()> {
        self.child.kill()
    }

    /// The child process PID, if available.
    pub fn process_id(&self) -> Option<u32> {
        self.child.process_id()
    }
}
