/// use with `syscalls` crate's `thread` feature.

use core::str;
use std::future::Future;
use std::os::fd::AsRawFd;
use std::pipe::{pipe, PipeReader, PipeWriter};
use std::task::{Context, Waker};
use syscalls::{sys_read, sys_write};

pub fn pipe_test() {
    let (pipe_reader, pipe_writer) = pipe().unwrap();
    let mut buf = [0; 1024];
    // let _ = reader(&pipe_reader, &mut buf); // 在阻塞式系统调用的情景下，这句代码会阻塞整个pipe_test函数，导致无法进行后续写入操作并唤醒该任务。
    let _ = writer(&pipe_writer);
    std::thread::sleep(core::time::Duration::from_millis(20));
    let _ = reader(&pipe_reader, &mut buf);
    println!("pipetest ok!");
}

fn reader(pipe_reader: &PipeReader, mut buf: &mut [u8]) {
    let n = sys_read(pipe_reader.as_raw_fd(), &mut buf).unwrap();
    println!("read {} bytes: {:?}", n, str::from_utf8(&buf[..n]));
}

fn writer(pipe_writer: &PipeWriter) {
    sys_write(pipe_writer.as_raw_fd(), b"Hello, world!").unwrap();
}