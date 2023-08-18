use nix::errno::Errno;
use std::fs::File;
use std::io::ErrorKind;
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::process::exit;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use v4l::buffer::Type;
use v4l::io::mmap::Stream;
use v4l::io::traits::CaptureStream;
use v4l::video::Capture;
use v4l::Device;
use v4l::FourCC;
mod pipe;

fn get_four_bytes(s: &String) -> Option<&[u8; 4]> {
    let bytes = s.as_bytes();
    bytes.get(..4).and_then(|slice| {
        if slice.len() == 4 {
            let array_ref: &[u8; 4] = slice.try_into().ok()?;
            Some(array_ref)
        } else {
            None
        }
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 3 {
        eprintln!(
            "Usage: {} /dev/videoX outfile [width height framerate pixelformat max_frames]",
            args[0]
        );
        exit(1);
    }
    let devname = &args[1];
    let out_file = &args[2];
    let mut width: u32 = 640;
    let mut height: u32 = 480;
    let mut framerate: u32 = 30;
    let mut pixelformat = b"MJPG";
    let mut max_frames: usize = 0;
    if args.len() >= 4 {
        width = args[3].parse().expect("failed to parse width");
    }
    if args.len() >= 5 {
        height = args[4].parse().expect("failed to parse height");
    }
    if args.len() >= 6 {
        framerate = args[5].parse().expect("failed to parse framerate");
    }
    if args.len() >= 7 {
        pixelformat = get_four_bytes(&args[6]).expect("failed to parse pixelformat");
    }
    if args.len() >= 8 {
        max_frames = args[7].parse().expect("failed to parse maxframes");
    }
    let mut writer =
        File::create(out_file).unwrap_or_else(|_| panic!("failed to open :{}", out_file));
    let mut output_to_pipe = false;
    if pipe::is_pipe(writer.as_raw_fd()) {
        output_to_pipe = true;
        match pipe::set_pipe_max_size(writer.as_raw_fd()) {
            Ok(_) => {}
            Err(e) => eprintln!("set_pipe_max_size:{e} (ignored)"),
        }
    }
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");
    let dev = Device::with_path(devname).expect("Failed to open device");

    let mut fmt = dev.format().expect("Failed to read format");
    fmt.width = width;
    fmt.height = height;
    fmt.fourcc = FourCC::new(pixelformat);
    let fmt = dev.set_format(&fmt).expect("Failed to write format");
    let mut params = dev.params().expect("Failed to read params");
    params.interval = v4l::fraction::Fraction {
        numerator: 1,
        denominator: framerate,
    };
    let params = dev.set_params(&params).expect("Failed to set params");

    // The actual format chosen by the device driver may differ from what we
    // requested! Print it out to get an idea of what is actually used now.
    eprintln!("Format in use:\n{}", fmt);
    eprintln!("Params in use:\n{}", params);

    let mut stream =
        Stream::with_buffers(&dev, Type::VideoCapture, 4).expect("Failed to create buffer stream");

    let mut frame_count: usize = 0;
    while running.load(Ordering::SeqCst) {
        match stream.next() {
            Ok(t) => {
                let (buf, meta) = t;
                eprintln!(
                    "Buffer size: {}, seq: {}, timestamp: {}",
                    buf.len(),
                    meta.sequence,
                    meta.timestamp
                );

                if output_to_pipe {
                    match pipe::vmsplice_single_buffer(buf, writer.as_raw_fd()) {
                        Ok(_) => {}
                        Err(e) if e == Errno::EPIPE => break,
                        Err(e) => {
                            eprintln!("error: {e:?}");
                            break;
                        }
                    }
                } else {
                    match writer.write_all(buf) {
                        Ok(_) => {}
                        Err(ref e) if e.kind() == ErrorKind::Interrupted => {}
                        Err(ref e) if e.kind() == ErrorKind::BrokenPipe => break,
                        Err(e) => {
                            eprintln!("error: {e:?}");
                            break;
                        }
                    }
                }
                frame_count += 1;
                if max_frames > 0 && frame_count >= max_frames {
                    break;
                }
            }
            Err(ref e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) => {
                println!("raw OS error: {e:?}");
                break;
            }
        }
    }
}
