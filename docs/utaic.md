# utaic

用户程序如何使用 taic 控制器

1. 先在用户态实现一个简单的运行时，包括 Task、Scheduler 等定义
2. 在初始化时使用系统调用获取到该进程能够使用的控制器的虚拟地址
3. 使用控制器进行调度以及唤醒