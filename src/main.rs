use std::process::exit;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use v4l::buffer::Type;
use v4l::io::mmap::Stream;
use v4l::io::traits::CaptureStream;
use v4l::video::Capture;
use v4l::Device;
use v4l::FourCC;

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
    let _out_file = &args[2];
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
    _ = max_frames;

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
    _ = framerate;

    // The actual format chosen by the device driver may differ from what we
    // requested! Print it out to get an idea of what is actually used now.
    eprintln!("Format in use:\n{}", fmt);

    let mut stream =
        Stream::with_buffers(&dev, Type::VideoCapture, 4).expect("Failed to create buffer stream");

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
            }
            Err(e) => {
                if let Some(raw_os_err) = e.raw_os_error() {
                    println!("raw OS error: {raw_os_err:?}");
                } else {
                    println!("Not an OS error");
                }
                //eprintln!("e = {}", e.what);
                break;
            }
        }
    }
}
