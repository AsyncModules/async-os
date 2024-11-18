/// 这里使用 read 来作为第一个异步系统调用的例子
/// 
/// 但在此之前，是否要提供注册和取消异步系统调用的同步系统调用接口呢？
/// 内核默认提供异步系统调用的环境，在必要的时候，才禁用异步系统调用
/// 异步不一定需要多核，单核也可以异步，单核情况下，通过 ecall 来通知内核
/// 

use crate::{SyscallFuture, Sysno};

pub fn sys_read(fd: i32, buf: &mut [u8]) -> SyscallFuture {
    run_sysfut(SyscallFuture::new(Sysno::read, &[fd as usize, buf.as_mut_ptr() as usize, buf.len()]))
}

pub fn sys_write(fd: i32, buf: &[u8]) -> SyscallFuture {
    run_sysfut(SyscallFuture::new(Sysno::write, &[fd as usize, buf.as_ptr() as usize, buf.len()]))
}

/// 用于预处理新建的`SyscallFuture`，使系统调用接口支持`await`和`non-await`两种调用方法
/// 如果使能了`thread` feature，则会在该函数内部（也就是各个系统调用接口内部）陷入内核执行系统调用，并得到带有结果的`SyscallFuture`；
/// 如果未使能`thread` feature，则该函数不会做任何处理，原样返回不带结果的`SyscallFuture`，协程通过`.await`调用该Future并得到结果。
fn run_sysfut(mut sf: SyscallFuture) -> SyscallFuture {
    // cfg_if::cfg_if! {
    //     if #[cfg(feature = "thread")] {
    //         sf.run(); // 在阻塞的系统调用模式下，一次调用一定能返回结果
    //         sf
    //     }
    //     else if #[cfg(not(feature = "thread"))] {
    //         sf
    //     }
    // }
    #[cfg(feature = "thread")]
    sf.run(); // 在阻塞的系统调用模式下，一次调用一定能返回结果
    sf
}
