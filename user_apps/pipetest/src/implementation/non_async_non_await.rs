/// use with `syscalls` crate's `thread` feature.

use core::str;
use std::os::fd::AsRawFd;
use std::pipe::{pipe, PipeReader, PipeWriter};
use syscalls::{sys_read, sys_write, Errno};

#[cfg(feature = "blocking")]
static IS_BLOCKING: &str = "blocking";
#[cfg(not(feature = "blocking"))]
static IS_BLOCKING: &str = "non-blocking";

pub fn pipe_test() {
    println!("pipe test: non-async, non-await, {}", IS_BLOCKING);

    let (pipe_reader, pipe_writer) = pipe().unwrap();
    let mut buf = [0; 1024];
    #[cfg(not(feature = "blocking"))]
    {
        let ta = std::thread::spawn(move || { reader(pipe_reader, &mut buf); });
        let tb = std::thread::spawn(move || { writer(pipe_writer); });
        ta.join();
        tb.join();
    }
    #[cfg(feature = "blocking")]
    {
        let tb = std::thread::spawn(move || { writer(pipe_writer); });
        let ta = std::thread::spawn(move || { reader(pipe_reader, &mut buf); });
        ta.join();
        tb.join();
    }
    println!("pipetest ok!");
}

fn reader(pipe_reader: PipeReader, mut buf: &mut [u8]) {
    let sysres = sys_read(pipe_reader.as_raw_fd(), &mut buf);
    loop {
        match *sysres {
            Ok(n) => {
                println!("read {} bytes: {:?}", n, str::from_utf8(&buf[..n]));
                return;
            },
            #[cfg(not(feature = "blocking"))]
            Err(Errno::EAGAIN) => {
                println!("syscall receive EAGAIN");
                std::thread::yield_now();
            },
            _ => {
                panic!("unsupported error.");
            }
        }
    }
}

fn writer(pipe_writer: PipeWriter) {
    let res = sys_write(pipe_writer.as_raw_fd(), b"Hello, world!").unwrap();
    println!("{:?}", res);
}