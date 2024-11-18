use core::str;
use std::future::Future;
use std::os::fd::AsRawFd;
use std::pipe::{pipe, PipeReader, PipeWriter};
use std::task::{Context, Waker};
use syscalls::{sys_read, sys_write};

pub fn pipe_test() {
    let (pipe_reader, pipe_writer) = pipe().unwrap();
    let mut buf = [0; 1024];
    let a = reader(pipe_reader, &mut buf);
    let b = writer(pipe_writer);
    let waker = Waker::noop();
    let mut cx = Context::from_waker(&waker);
    let mut futa = std::mem::ManuallyDrop::new(Box::pin(a));
    let mut futb = std::mem::ManuallyDrop::new(Box::pin(b));
    let _ = futa.as_mut().poll(&mut cx);
    let _ = futb.as_mut().poll(&mut cx);
    std::thread::sleep(core::time::Duration::from_millis(20));
    let _ = futa.as_mut().poll(&mut cx);

    println!("pipetest ok!");
}

async fn reader(pipe_reader: PipeReader, mut buf: &mut [u8]) {
    let n = sys_read(pipe_reader.as_raw_fd(), &mut buf).await.unwrap();
    println!("read {} bytes: {:?}", n, str::from_utf8(&buf[..n]));
}

async fn writer(pipe_writer: PipeWriter) {
    sys_write(pipe_writer.as_raw_fd(), b"Hello, world!").await.unwrap();
}