#[cfg(feature = "async-await")]
mod async_await;
#[cfg(feature = "async-await")]
pub use async_await::pipe_test;

#[cfg(feature = "async-non-await")]
mod async_non_await;
#[cfg(feature = "async-non-await")]
pub use async_non_await::pipe_test;

#[cfg(feature = "non-async-non-await")]
mod non_async_non_await;
#[cfg(feature = "non-async-non-await")]
pub use non_async_non_await::pipe_test;