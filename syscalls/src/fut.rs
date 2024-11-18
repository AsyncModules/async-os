use crate::{raw, Errno, Sysno};
use core::{
    cell::Cell,
    future::Future,
    ops::Deref,
    pin::Pin,
    task::{Context, Poll}
};
use alloc::vec::Vec;

#[repr(C)]
pub struct SyscallRes(Cell<Option<Result<usize, Errno>>>);

impl SyscallRes {

    pub fn get_ptr(&mut self) -> *mut Option<Result<usize, Errno>> {
        self.0.as_ptr()
    }

    pub fn replace(&mut self, res: Result<usize, Errno>) {
        self.0.set(Some(res));
    }

    pub fn get(&mut self) -> Option<Result<usize, Errno>> {
        self.0.get()
    }
}

pub struct SyscallFuture {
    pub has_issued: bool,
    pub id: Sysno,
    pub args: Vec<usize>,
    pub res: SyscallRes,
}

impl SyscallFuture {

    pub fn new(id: Sysno, args: &[usize]) -> Self {
        Self { has_issued: false, id, args: Vec::from(args), res: SyscallRes(Cell::new(None)) }
    }

    pub(crate) fn run(&mut self) {
        // 目前仍然是通过 ecall 来发起系统调用
        let _ret_ptr = self.res.get_ptr() as *mut usize as usize;
        // 需要新增一个参数来记录返回值的位置
        // 详细的设置见 crate::raw_syscall
        #[cfg(target_arch = "riscv64")]
        let res = unsafe { match self.args.len() {
            0 => raw::syscall0(self.id as _, _ret_ptr),
            1 => raw::syscall1(self.id as _, self.args[0], _ret_ptr),
            2 => raw::syscall2(self.id as _, self.args[0], self.args[1], _ret_ptr),
            3 => raw::syscall3(
                self.id as _, 
                self.args[0], 
                self.args[1], 
                self.args[2],
                _ret_ptr
            ),
            4 => raw::syscall4(
                self.id as _, 
                self.args[0], 
                self.args[1], 
                self.args[2], 
                self.args[3],
                _ret_ptr
            ),
            5 => raw::syscall5(
                self.id as _, 
                self.args[0], 
                self.args[1], 
                self.args[2], 
                self.args[3], 
                self.args[4],
                _ret_ptr
            ),
            6 => raw::syscall6(
                self.id as _, 
                self.args[0], 
                self.args[1], 
                self.args[2], 
                self.args[3], 
                self.args[4], 
                self.args[5],
                _ret_ptr
            ),
            _ => panic!("not support the number of syscall args > 6"),
        }};
        #[cfg(not(target_arch = "riscv64"))]
        let res = unsafe { match self.args.len() {
            0 => raw::syscall0(self.id as _),
            1 => raw::syscall1(self.id as _, self.args[0]),
            2 => raw::syscall2(self.id as _, self.args[0], self.args[1]),
            3 => raw::syscall3(
                self.id as _, 
                self.args[0], 
                self.args[1], 
                self.args[2],
            ),
            4 => raw::syscall4(
                self.id as _, 
                self.args[0], 
                self.args[1], 
                self.args[2], 
                self.args[3],
            ),
            5 => raw::syscall5(
                self.id as _, 
                self.args[0], 
                self.args[1], 
                self.args[2], 
                self.args[3], 
                self.args[4],
            ),
            6 => raw::syscall6(
                self.id as _, 
                self.args[0], 
                self.args[1], 
                self.args[2], 
                self.args[3], 
                self.args[4], 
                self.args[5],
            ),
            _ => panic!("not support the number of syscall args > 6"),
        }};
        if res as i32 != Errno::EAGAIN.into_raw()  {
            self.res.replace(Errno::from_ret(res));
        }
    }
}

impl Future for SyscallFuture {
    type Output = Result<usize, Errno>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if let Some(ret) = this.res.get() {
            return Poll::Ready(ret);
        } else {
            if !this.has_issued {
                this.has_issued = true;
                this.run();
            }
            if let Some(ret) = this.res.get() {
                return Poll::Ready(ret);
            } else {
                return Poll::Pending;
            }
        }
    }
}

#[cfg(feature = "thread")]
impl Deref for SyscallFuture {
    type Target = Result<usize, Errno>;

    fn deref(&self) -> &Self::Target {
        unsafe {
            &(&*self.res.0.as_ptr()).as_ref().unwrap()
        }
    }
}