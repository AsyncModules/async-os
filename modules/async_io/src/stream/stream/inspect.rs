use core::pin::Pin;

use pin_project_lite::pin_project;

use super::AsyncStream;
use core::task::{Context, Poll};

pin_project! {
    /// A stream that does something with each element of another stream.
    ///
    /// This `struct` is created by the [`inspect`] method on [`Stream`]. See its
    /// documentation for more.
    ///
    /// [`inspect`]: trait.Stream.html#method.inspect
    /// [`Stream`]: trait.Stream.html
    #[derive(Debug)]
    pub struct Inspect<S, F> {
        #[pin]
        stream: S,
        f: F,
    }
}

impl<S, F> Inspect<S, F> {
    pub(super) fn new(stream: S, f: F) -> Self {
        Self { stream, f }
    }
}

impl<S, F> AsyncStream for Inspect<S, F>
where
    S: AsyncStream,
    F: FnMut(&S::Item),
{
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        let next = core::task::ready!(this.stream.as_mut().poll_next(cx));

        Poll::Ready(next.map(|x| {
            (this.f)(&x);
            x
        }))
    }
}
