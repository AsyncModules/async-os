use core::future::Future;
use core::mem;
use core::pin::Pin;

use crate::{self as io, AsyncWrite};
use core::task::{Context, Poll};

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct WriteAllFuture<'a, T: Unpin + ?Sized> {
    pub(crate) writer: &'a mut T,
    pub(crate) buf: &'a [u8],
}

impl<T: AsyncWrite + Unpin + ?Sized> Future for WriteAllFuture<'_, T> {
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { writer, buf } = &mut *self;

        while !buf.is_empty() {
            let n = core::task::ready!(Pin::new(&mut **writer).write(cx, buf))?;
            let (_, rest) = mem::replace(buf, &[]).split_at(n);
            *buf = rest;

            if n == 0 {
                return Poll::Ready(Err(io::Error::WriteZero));
            }
        }

        Poll::Ready(Ok(()))
    }
}
