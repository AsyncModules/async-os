//! 这个仓库对原来的 syscall 结果进行封装，提供同步和异步的系统调用接口
//!
//!
//!
//!
#![no_std]
#![allow(unused)]

extern crate alloc;

mod fut;
mod raw_syscall;
mod syscall;

pub use fut::SyscallFuture;
pub use syscall::*;

use syscalls::Sysno;
use syscalls::Errno;

pub mod raw {
    //! Exposes raw syscalls that simply return a `usize` instead of a `Result`.
    pub use super::raw_syscall::*;
}

pub(crate) const ASYNC_FLAG: usize = 0x5f5f5f5f;

#[cfg(feature = "thread")]
pub(crate) const IS_ASYNC: usize = 0;
#[cfg(not(feature = "thread"))]
pub(crate) const IS_ASYNC: usize = ASYNC_FLAG;