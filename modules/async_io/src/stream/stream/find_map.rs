use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use super::AsyncStream;

#[doc(hidden)]
pub struct FindMapFuture<'a, S, F> {
    stream: &'a mut S,
    f: F,
}

impl<'a, S, F> FindMapFuture<'a, S, F> {
    pub(super) fn new(stream: &'a mut S, f: F) -> Self {
        Self { stream, f }
    }
}

impl<S: Unpin, F> Unpin for FindMapFuture<'_, S, F> {}

impl<'a, S, B, F> Future for FindMapFuture<'a, S, F>
where
    S: AsyncStream + Unpin + Sized,
    F: FnMut(S::Item) -> Option<B>,
{
    type Output = Option<B>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let item = core::task::ready!(Pin::new(&mut *self.stream).poll_next(cx));

        match item {
            Some(v) => match (&mut self.f)(v) {
                Some(v) => Poll::Ready(Some(v)),
                None => Poll::Pending,
            },
            None => Poll::Ready(None),
        }
    }
}
