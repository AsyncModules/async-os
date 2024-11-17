# control_flow

涉及trap或任务切换的几个过程的函数执行流。

## 模块初始化到执行第一个任务

`modules/arch_boot/src/platform/riscv64_qemu_virt/mod.rs:rust_entry` 

--call-> 

`modules/runtime/src/lib.rs:rust_main`：初始化内核executor，将async_main包装后的main_fut放入内核Executor

--return->

`rust_entry`

--call->

`modules/trampoline/src/lib.rs:trampoline`：取出当前任务（即main_fut）

--call->

`modules/trampoline/src/lib.rs:run_task`：取出任务中的future，poll它并处理结果

## 进程从创建到执行

`modules/trampoline/src/executor_api.rs:init_user`

--call->

`modules/executor/src/executor.rs:Executor::init_user`：创建用户进程（executor）和其中的主协程，将该executor的run协程放入内核executor

--内核调度运行该run协程->

`modules/executor/src/executor.rs:Executor::run`：切换地址空间和调度器

--（当前调度器已变为用户executor）调度运行进程的主协程->

`modules/trampoline/src/task_api.rs:user_task_top`：（该async函数作为用户态任务的Future上下文）但在此过程中没有作用

--return（从协程返回到executor）->

`modules/trampoline/src/lib.rs:run_task`：使用sret恢复到用户态执行流

## 内核任务切换

前一个协程返回->

`modules/trampoline/src/lib.rs:run_task`

--return->

`modules/trampoline/src/lib.rs:trampoline`：取出下一任务

--call->

`run_task`：运行下一任务

## 用户态trap

发生中断、异常或系统调用->

硬件设置寄存器、跳转到中断处理程序

--jump->

`modules/trampoline/src/arch/riscv/mod.rs:trap_vector_base`：保存TrapFrame

--call->

`modules/trampoline/src/lib.rs:trampoline`

--call->

`modules/trampoline/src/lib.rs:run_task`：执行用户执行流对应的内核协程`user_task_top`

--call（运行协程）->

`modules/trampoline/src/task_api.rs:user_task_top`：进行Trap处理等工作

--return（从协程返回到executor）->

`run_task`：使用sret恢复到用户态执行流

## 内核态trap

发生中断、异常或系统调用->

硬件设置寄存器、跳转到中断处理程序

--jump->

`modules/trampoline/src/arch/riscv/mod.rs:trap_vector_base`：保存TrapFrame

--call->

`modules/trampoline/src/lib.rs:trampoline`：直接在此函数中处理Trap

--return-->

`modules/trampoline/src/arch/riscv/mod.rs:trap_vector_base`

--sret-->

回到之前正在执行的内核代码

## 异步系统调用

`syscalls/src/fut.rs:SyscallFuture::poll`：系统调用Future的poll函数

--call->

`syscalls/src/fut.rs:SyscallFuture::run`：传入系统调用参数，其中包括异步标记和SyscallFuture的结果地址

--根据“用户态trap”的路径陷入内核态->

`modules/trampoline/src/task_api.rs:user_task_top`：确认需要以异步方式处理系统调用

-->

创建系统调用处理协程，修改当前任务为该处理协程，并通过poll手动调用它

（如果单次poll未能处理完成，则保存的waker是处理协程的waker，会在相关资源可用时唤醒该处理协程）

-->

将当前任务修改回`user_task_top`

--根据“用户态trap”的路径恢复到用户态->

`run`：如果系统调用处理未完成，则返回的结果为`EAGAIN`，不会记录在SyscallFuture的结果中

--return->

`poll`：如果系统调用处理未完成，则poll函数没有查询到结果，返回Pending；否则，返回Ready。