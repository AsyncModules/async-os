//! 负责处理进程中与信号相关的内容
extern crate alloc;
use crate::KERNEL_EXECUTOR_ID;
use alloc::sync::Arc;
use axerrno::{AxError, AxResult};
use axhal::cpu::this_cpu_id;
use axlog::{info, warn};
use axsignal::{
    action::{SigActionFlags, SignalDefault, SIG_DFL, SIG_IGN},
    info::SigInfo,
    signal_no::SignalNo,
    ucontext::{SignalStack, SignalUserContext},
    SignalHandler, SignalSet,
};
use sync::Mutex;
use taskctx::TrapFrame;

/// 信号处理模块，进程间不共享
pub struct SignalModule {
    /// 是否存在siginfo
    pub sig_info: bool,
    /// 保存的trap上下文
    pub last_trap_frame_for_signal: Option<TrapFrame>,
    /// 信号处理函数集
    pub signal_handler: Arc<Mutex<SignalHandler>>,
    /// 未决信号集
    pub signal_set: SignalSet,
    /// exit signal
    exit_signal: Option<SignalNo>,
    /// Alternative signal stack
    pub alternate_stack: SignalStack,
}

impl SignalModule {
    /// 初始化信号模块
    pub fn init_signal(signal_handler: Option<Arc<Mutex<SignalHandler>>>) -> Self {
        let signal_handler =
            signal_handler.unwrap_or_else(|| Arc::new(Mutex::new(SignalHandler::new())));
        let signal_set = SignalSet::new();
        let last_trap_frame_for_signal = None;
        let sig_info = false;
        Self {
            sig_info,
            last_trap_frame_for_signal,
            signal_handler,
            signal_set,
            exit_signal: None,
            alternate_stack: SignalStack::default(),
        }
    }

    /// Judge whether the signal request the interrupted syscall to restart
    ///
    /// # Return
    /// - None: There is no siganl need to be delivered
    /// - Some(true): The interrupted syscall should be restarted
    /// - Some(false): The interrupted syscall should not be restarted
    pub async fn have_restart_signal(&self) -> Option<bool> {
        match self.signal_set.find_signal() {
            Some(sig_num) => Some(
                self.signal_handler
                    .lock()
                    .await
                    .get_action(sig_num)
                    .need_restart(),
            ),

            None => None,
        }
    }

    pub fn set_exit_signal(&mut self, signal: SignalNo) {
        self.exit_signal = Some(signal);
    }

    pub fn get_exit_signal(&self) -> Option<SignalNo> {
        self.exit_signal
    }
}

const USER_SIGNAL_PROTECT: usize = 512;

use crate::{current_executor, current_task, exit, PID2PC, TID2TASK};

/// 将保存的trap上下文填入内核栈中
///
/// 若使用了SIG_INFO，此时会对原有trap上下文作一定修改。
///
/// 若确实存在可以被恢复的trap上下文，则返回true
#[no_mangle]
pub async fn load_trap_for_signal() -> bool {
    let current_process = current_executor().await;
    let current_task = current_task();

    let mut signal_modules = current_process.signal_modules.lock().await;
    let signal_module = signal_modules.get_mut(&current_task.id().as_u64()).unwrap();
    if let Some(old_trap_frame) = signal_module.last_trap_frame_for_signal.take() {
        unsafe {
            // let now_trap_frame: *mut TrapFrame = current_task.get_first_trap_frame();

            let now_trap_frame = current_task.utrap_frame().unwrap();
            // let mut now_trap_frame =
            //     read_trapframe_from_kstack(current_task.get_kernel_stack_top().unwrap());
            // 考虑当时调用信号处理函数时，sp对应的地址上的内容即是SignalUserContext
            // 此时认为一定通过sig_return调用这个函数
            // 所以此时sp的位置应该是SignalUserContext的位置
            let sp = now_trap_frame.get_sp();
            *now_trap_frame = old_trap_frame;
            if signal_module.sig_info {
                let pc = (*(sp as *const SignalUserContext)).get_pc();
                now_trap_frame.set_pc(pc);
            }
        }
        true
    } else {
        false
    }
}

/// 处理 Terminate 类型的信号
async fn terminate_process(signal: SignalNo, info: Option<SigInfo>) {
    let current_task = current_task();
    warn!("Terminate process: {}", current_task.get_process_id());
    if current_task.is_leader() {
        // exit_current_task(signal as i32);
        exit(signal as isize).await;
    } else {
        // 此时应当关闭当前进程
        // 选择向主线程发送信号内部来关闭
        send_signal_to_process(
            current_task.get_process_id() as isize,
            signal as isize,
            info,
        )
        .await
        .unwrap();
        // exit_current_task(-1);
        exit(-1).await;
    }
}

/// 处理当前进程的信号
///
/// 若返回值为真，代表需要进入处理信号，因此需要执行trap的返回
pub async fn handle_signals() {
    let process = current_executor().await;
    let current_task = current_task();
    if let Some(signal_no) = current_task.check_pending_signal() {
        send_signal_to_thread(current_task.id().as_u64() as isize, signal_no as isize)
            .await
            .unwrap_or_else(|err| {
                warn!("send signal failed: {:?}", err);
            });
    }
    if process.get_zombie() {
        if current_task.is_leader() {
            return;
        }
        // 进程退出了，在测试环境下非主线程应该立即退出
        // exit_current_task(0);
        exit(0).await;
    }

    if process.pid() == KERNEL_EXECUTOR_ID {
        // 内核进程不处理信号
        return;
    }
    let mut signal_modules = process.signal_modules.lock().await;

    let signal_module = signal_modules.get_mut(&current_task.id().as_u64()).unwrap();
    let signal_set = &mut signal_module.signal_set;
    let sig_num = if let Some(sig_num) = signal_set.get_one_signal() {
        sig_num
    } else {
        return;
    };
    info!(
        "cpu: {}, task: {}, handler signal: {}",
        this_cpu_id(),
        current_task.id().as_u64(),
        sig_num
    );
    let signal = SignalNo::from(sig_num);
    let mask = signal_set.mask;
    // 存在未被处理的信号
    if signal_module.last_trap_frame_for_signal.is_some() {
        // 之前的trap frame还未被处理
        // 说明之前的信号处理函数还未返回，即出现了信号嵌套。
        if signal == SignalNo::SIGSEGV || signal == SignalNo::SIGBUS {
            // 在处理信号的过程中又触发 SIGSEGV 或 SIGBUS，此时会导致死循环，所以直接结束当前进程
            drop(signal_modules);
            // exit_current_task(-1);
            exit(-1).await;
        }
        return;
    }
    // 之前的trap frame已经被处理
    // 说明之前的信号处理函数已经返回，即没有信号嵌套。
    // 此时可以将当前的trap frame保存起来
    // signal_module.last_trap_frame_for_signal = Some(read_trapframe_from_kstack(
    //     current_task.get_kernel_stack_top().unwrap(),
    // ));
    signal_module.last_trap_frame_for_signal = Some(*current_task.utrap_frame().unwrap());
    // current_task.set_siginfo(false);
    signal_module.sig_info = false;
    // 调取处理函数
    let signal_handler = signal_module.signal_handler.lock().await;
    let action = signal_handler.get_action(sig_num);
    if action.sa_handler == SIG_DFL {
        drop(signal_handler);
        drop(signal_modules);
        // 未显式指定处理函数，使用默认处理函数
        match SignalDefault::get_action(signal) {
            SignalDefault::Ignore => {
                // 忽略，此时相当于已经完成了处理，所以要把trap上下文清空
                load_trap_for_signal().await;
            }
            SignalDefault::Terminate => {
                terminate_process(signal, None).await;
            }
            SignalDefault::Stop => {
                unimplemented!();
            }
            SignalDefault::Cont => {
                unimplemented!();
            }
            SignalDefault::Core => {
                terminate_process(signal, None).await;
            }
        }
        return;
    }
    if action.sa_handler == SIG_IGN {
        // 忽略处理
        return;
    }
    // 此时需要调用信号处理函数，注意调用的方式是：
    // 通过修改trap上下文的pc指针，使得trap返回之后，直接到达信号处理函数
    // 因此需要处理一系列的trap上下文，使得正确传参与返回。
    // 具体来说需要考虑两个方面：
    // 1. 传参
    // 2. 返回值ra地址的设定，与是否设置了SA_RESTORER有关

    // 读取当前的trap上下文
    // let mut trap_frame = read_trapframe_from_kstack(current_task.get_kernel_stack_top().unwrap());
    let trap_frame = current_task.utrap_frame().unwrap();

    // // 新的trap上下文的sp指针位置，由于SIGINFO会存放内容，所以需要开个保护区域
    let mut sp = if action.sa_flags.contains(SigActionFlags::SA_ONSTACK)
        && signal_module.alternate_stack.flags != axsignal::ucontext::SS_DISABLE
    {
        axlog::debug!("Use alternate stack");
        // Use alternate stack
        (signal_module.alternate_stack.sp + signal_module.alternate_stack.size - 1) & !0xf
    } else {
        trap_frame.get_sp() - USER_SIGNAL_PROTECT
    };

    info!("use stack: {:#x}", sp);
    let restorer = if let Some(addr) = action.get_storer() {
        addr
    } else {
        axconfig::SIGNAL_TRAMPOLINE
    };

    info!(
        "restorer :{:#x}, handler: {:#x}",
        restorer, action.sa_handler
    );
    #[cfg(not(target_arch = "x86_64"))]
    trap_frame.set_ra(restorer);

    let old_pc = trap_frame.get_pc();

    trap_frame.set_pc(action.sa_handler);
    // 传参
    trap_frame.set_arg0(sig_num);
    // 若带有SIG_INFO参数，则函数原型为fn(sig: SignalNo, info: &SigInfo, ucontext: &mut UContext)
    if action.sa_flags.contains(SigActionFlags::SA_SIGINFO) {
        // current_task.set_siginfo(true);
        signal_module.sig_info = true;
        let sp_base = (((sp - core::mem::size_of::<SigInfo>()) & !0xf)
            - core::mem::size_of::<SignalUserContext>())
            & !0xf;

        // TODO: 统一为访问用户空间的操作封装函数
        process
            .manual_alloc_range_for_lazy(sp_base.into(), sp.into())
            .await
            .expect("Failed to alloc memory for signal user stack");

        // 注意16字节对齐
        sp = (sp - core::mem::size_of::<SigInfo>()) & !0xf;
        let info = if let Some(info) = signal_set.info.get(&(sig_num - 1)) {
            info!("test SigInfo: {:?}", info.0.si_val_int);
            info.0
        } else {
            SigInfo {
                si_signo: sig_num as i32,
                ..Default::default()
            }
        };
        unsafe {
            *(sp as *mut SigInfo) = info;
        }
        trap_frame.set_arg1(sp);

        // 接下来存储ucontext
        sp = (sp - core::mem::size_of::<SignalUserContext>()) & !0xf;

        let ucontext = SignalUserContext::init(old_pc, mask);
        unsafe {
            *(sp as *mut SignalUserContext) = ucontext;
        }
        trap_frame.set_arg2(sp);
    }

    #[cfg(target_arch = "x86_64")]
    unsafe {
        // set return rip
        sp -= core::mem::size_of::<usize>();
        *(sp as *mut usize) = restorer;
    }

    trap_frame.set_user_sp(sp);
    // 将修改后的trap上下文写回内核栈
    // write_trapframe_to_kstack(current_task.get_kernel_stack_top().unwrap(), &trap_frame);
    drop(signal_handler);
    drop(signal_modules);
}

/// 从信号处理函数返回
///
/// 返回的值与原先syscall应当返回的值相同，即返回原先保存的trap上下文的a0的值
pub async fn signal_return() -> isize {
    if load_trap_for_signal().await {
        // 说明确实存在着信号处理函数的trap上下文
        // 此时内核栈上存储的是调用信号处理前的trap上下文
        current_task().utrap_frame().unwrap().get_ret_code() as isize
        // read_trapframe_from_kstack(current_task().get_kernel_stack_top().unwrap()).get_ret_code()
        //     as isize
    } else {
        // 没有进行信号处理，但是调用了sig_return
        // 此时直接返回-1
        -1
    }
}

/// 发送信号到指定的进程
///
/// 默认发送到该进程下的主线程
pub async fn send_signal_to_process(
    pid: isize,
    signum: isize,
    info: Option<SigInfo>,
) -> AxResult<()> {
    let mut pid2pc = PID2PC.lock().await;
    if !pid2pc.contains_key(&(pid as u64)) {
        return Err(axerrno::AxError::NotFound);
    }
    let process = pid2pc.get_mut(&(pid as u64)).unwrap();
    let main_task = process.get_main_task().await;
    if let Some(main_task) = main_task {
        let mut signal_modules = process.signal_modules.lock().await;
        let signal_module = signal_modules.get_mut(&main_task.id().as_u64()).unwrap();
        signal_module
            .signal_set
            .try_add_signal(signum as usize, info);
        // 如果这个时候对应的线程是处于休眠状态的，则唤醒之，进入信号处理阶段
        if main_task.is_blocked() {
            taskctx::wakeup_task(Arc::as_ptr(&main_task));
        }
    }
    // let mut now_id: Option<u64> = None;
    // for task in process.tasks.lock().iter_mut() {
    //     if task.is_leader() {
    //         now_id = Some(task.id().as_u64());
    //         break;
    //     }
    // }
    // if now_id.is_some() {
    //     let mut signal_modules = process.signal_modules.lock().await;
    //     let signal_module = signal_modules.get_mut(&now_id.unwrap()).unwrap();
    //     signal_module
    //         .signal_set
    //         .try_add_signal(signum as usize, info);
    //     let tid2task = TID2TASK.lock().await;
    //     let main_task = Arc::clone(tid2task.get(&now_id.unwrap()).unwrap());
    //     // 如果这个时候对应的线程是处于休眠状态的，则唤醒之，进入信号处理阶段
    //     if main_task.is_blocked() {
    //         // axtask::wakeup_task(main_task);
    //         taskctx::wakeup_task(main_task);
    //     }
    // }
    Ok(())
}

/// 发送信号到指定的线程
pub async fn send_signal_to_thread(tid: isize, signum: isize) -> AxResult<()> {
    let tid2task = TID2TASK.lock().await;
    let task = if let Some(task) = tid2task.get(&(tid as u64)) {
        Arc::clone(task)
    } else {
        return Err(AxError::NotFound);
    };
    drop(tid2task);
    let pid = task.get_process_id();
    let pid2pc = PID2PC.lock().await;
    let process = if let Some(process) = pid2pc.get(&pid) {
        Arc::clone(process)
    } else {
        return Err(AxError::NotFound);
    };
    drop(pid2pc);
    let mut signal_modules = process.signal_modules.lock().await;
    if !signal_modules.contains_key(&(tid as u64)) {
        return Err(axerrno::AxError::NotFound);
    }
    let signal_module = signal_modules.get_mut(&(tid as u64)).unwrap();
    signal_module
        .signal_set
        .try_add_signal(signum as usize, None);
    // 如果这个时候对应的线程是处于休眠状态的，则唤醒之，进入信号处理阶段
    if task.is_blocked() {
        taskctx::wakeup_task(Arc::as_ptr(&task));
    }
    Ok(())
}

/// Whether the current process has signals pending
pub async fn current_have_signals() -> bool {
    current_executor().await.have_signals().await.is_some()
}
