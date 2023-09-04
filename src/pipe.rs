use nix::errno::Errno;
use nix::fcntl::{fcntl, FcntlArg};
use nix::fcntl::{vmsplice, SpliceFFlags};
use std::io::{self, IoSlice};
use std::os::unix::io::RawFd;

// Check if the given file descriptor is a pipe
pub fn is_pipe(fd: RawFd) -> bool {
    match nix::sys::stat::fstat(fd) {
        Ok(stat) => stat.st_mode & libc::S_IFMT == libc::S_IFIFO,
        Err(_) => false,
    }
}

// Get pipe max buffer size
#[cfg(target_os = "linux")]
pub fn get_pipe_max_size() -> Result<usize, io::Error> {
    // Read the maximum pipe size
    let pipe_max_size = std::fs::read_to_string("/proc/sys/fs/pipe-max-size")?;
    let max_size: usize = pipe_max_size.trim_end().parse().map_err(|err| {
        eprintln!("Failed to parse /proc/sys/fs/pipe-max-size: {:?}", err);
        io::Error::new(io::ErrorKind::InvalidData, "Failed to parse max pipe size")
    })?;
    Ok(max_size)
}

// Set the size of the given pipe file descriptor to the maximum size
#[cfg(target_os = "linux")]
pub fn set_pipe_max_size(fd: RawFd) -> Result<(), io::Error> {
    let max_size: libc::c_int = get_pipe_max_size()? as _;

    // If the current size is less than the maximum size, set the pipe size to the maximum size
    let current_size = fcntl(fd, FcntlArg::F_GETPIPE_SZ)?;
    if current_size < max_size {
        _ = fcntl(fd, FcntlArg::F_SETPIPE_SZ(max_size))?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn vmsplice_single_buffer(mut buf: &[u8], fd: RawFd) -> Result<(), io::Error> {
    if buf.is_empty() {
        return Ok(());
    };
    loop {
        let iov = IoSlice::new(buf);
        match vmsplice(fd, &[iov], SpliceFFlags::SPLICE_F_GIFT) {
            Ok(n) if n == iov.len() => return Ok(()),
            Ok(n) if n != 0 => buf = &buf[n..],
            Ok(_) => unreachable!(),
            Err(err) if err == Errno::EINTR => {}
            Err(err) => return Err(err.into()),
        }
    }
}
