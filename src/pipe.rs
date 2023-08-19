use nix::errno::Errno;
use nix::fcntl::{fcntl, FcntlArg};
use nix::fcntl::{vmsplice, SpliceFFlags};
use std::fs::File;
use std::io::IoSlice;
use std::io::{self, Read};
use std::os::unix::io::RawFd;

// Check if the given file descriptor is a pipe
pub fn is_pipe(fd: RawFd) -> bool {
    match nix::sys::stat::fstat(fd) {
        Ok(stat) => stat.st_mode & libc::S_IFMT == libc::S_IFIFO,
        Err(_) => false,
    }
}

// Get pipe max buffer size
pub fn get_pipe_max_size() -> Result<usize, io::Error> {
    // Read the maximum pipe size
    let mut pipe_max_size_file = File::open("/proc/sys/fs/pipe-max-size")?;
    let mut buffer = String::new();
    pipe_max_size_file.read_to_string(&mut buffer)?;
    let max_size_str = buffer.trim_end();
    let max_size: usize = max_size_str.parse().map_err(|err| {
        eprintln!("Failed to parse /proc/sys/fs/pipe-max-size: {:?}", err);
        io::Error::new(io::ErrorKind::InvalidData, "Failed to parse max pipe size")
    })?;
    Ok(max_size)
}

// Set the size of the given pipe file descriptor to the maximum size
pub fn set_pipe_max_size(fd: RawFd) -> Result<(), io::Error> {
    let max_size: libc::c_int = get_pipe_max_size()? as _;

    // If the current size is less than the maximum size, set the pipe size to the maximum size
    let current_size = fcntl(fd, FcntlArg::F_GETPIPE_SZ)?;
    if current_size < max_size {
        _ = fcntl(fd, FcntlArg::F_SETPIPE_SZ(max_size))?;
    }
    Ok(())
}

pub fn vmsplice_single_buffer(mut buf: &[u8], fd: RawFd) -> Result<(), Errno> {
    if buf.is_empty() {
        return Ok(());
    };
    let mut iov = IoSlice::new(buf);
    loop {
        match vmsplice(fd, &[iov], SpliceFFlags::SPLICE_F_GIFT) {
            Ok(n) => {
                if n == iov.len() {
                    return Ok(());
                } else if n != 0 {
                    buf = &buf[n..];
                    iov = IoSlice::new(buf);
                    continue;
                } else {
                    unreachable!();
                }
            }
            Err(err) => match err {
                Errno::EINTR => continue,
                _ => return Err(err),
            },
        }
    }
}
