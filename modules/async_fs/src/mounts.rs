use alloc::sync::Arc;
use async_vfs::{VfsNodeType, VfsOps, VfsResult, AsyncVfsOps, AsyncVfsNodeOps};

use crate::fs;

#[cfg(feature = "devfs")]
pub(crate) fn devfs() -> Arc<fs::devfs::DeviceFileSystem> {
    let null = fs::devfs::NullDev;
    let zero = fs::devfs::ZeroDev;
    let bar = fs::devfs::ZeroDev;
    let random = fs::devfs::RandomDev::default();
    let urandom = fs::devfs::RandomDev::default();

    let devfs = fs::devfs::DeviceFileSystem::new();
    let foo_dir = devfs.mkdir("foo");
    devfs.add("null", Arc::new(null));
    devfs.add("zero", Arc::new(zero));
    foo_dir.add("bar", Arc::new(bar));
    devfs.add("random", Arc::new(random));
    devfs.add("urandom", Arc::new(urandom));
    #[cfg(feature = "monolithic")]
    {
        // 添加dev文件系统下的配置文件
        // busybox的时候要用到
        // devfs不支持可修改的file，因此取巧直接用了ramfs提供的file实现
        let testrtc = fs::ramfs::FileNode::new();
        let _shm_dir = devfs.mkdir("shm");
        let rtc_dir = devfs.mkdir("misc");
        rtc_dir.add("rtc", Arc::new(testrtc));
    }
    Arc::new(devfs)
}

#[cfg(feature = "ramfs")]
pub(crate) fn ramfs() -> Arc<fs::ramfs::RamFileSystem> {
    Arc::new(fs::ramfs::RamFileSystem::new())
}

#[cfg(feature = "procfs")]
pub(crate) async fn procfs() -> VfsResult<Arc<fs::ramfs::RamFileSystem>> {
    let procfs = fs::ramfs::RamFileSystem::new();
    let proc_root = procfs.root_dir();

    // Create /proc/sys/net/core/somaxconn
    proc_root.create("sys", VfsNodeType::Dir).await?;
    proc_root.create("sys/net", VfsNodeType::Dir).await?;
    proc_root.create("sys/net/core", VfsNodeType::Dir).await?;
    proc_root.create("sys/net/core/somaxconn", VfsNodeType::File).await?;
    let file_somaxconn = proc_root.clone().lookup("./sys/net/core/somaxconn")?;
    file_somaxconn.write_at(0, b"4096\n").await?;

    // Create /proc/sys/vm/overcommit_memory
    proc_root.create("sys/vm", VfsNodeType::Dir).await?;
    proc_root.create("sys/vm/overcommit_memory", VfsNodeType::File).await?;
    let file_over = proc_root.clone().lookup("./sys/vm/overcommit_memory")?;
    file_over.write_at(0, b"0\n").await?;

    // Create /proc/self/stat
    proc_root.create("self", VfsNodeType::Dir).await?;
    proc_root.create("self/stat", VfsNodeType::File).await?;
    proc_root.create("self/exe", VfsNodeType::File).await?;
    proc_root.create("self/status", VfsNodeType::File).await?;

    // Create /proc/filesystems
    proc_root.create("filesystems", VfsNodeType::File).await?;

    #[cfg(feature = "monolithic")]
    {
        // Create other file to pass the testcases
        proc_root.create("meminfo", VfsNodeType::File).await?;
        proc_root.create("mounts", VfsNodeType::File).await?;
        proc_root.create("interrupts", VfsNodeType::File).await?;
        // procfs.mount("interrupts", Arc::new(fs::devfs::Interrupts::default()))?;
    }
    Ok(Arc::new(procfs))
}

#[cfg(feature = "sysfs")]
pub(crate) async fn sysfs() -> VfsResult<Arc<fs::ramfs::RamFileSystem>> {
    let sysfs = fs::ramfs::RamFileSystem::new();
    let sys_root = sysfs.root_dir();

    // Create /sys/kernel/mm/transparent_hugepage/enabled
    sys_root.create("kernel", VfsNodeType::Dir).await?;
    sys_root.create("kernel/mm", VfsNodeType::Dir).await?;
    sys_root.create("kernel/mm/transparent_hugepage", VfsNodeType::Dir).await?;
    sys_root.create("kernel/mm/transparent_hugepage/enabled", VfsNodeType::File).await?;
    let file_hp = sys_root
        .clone()
        .lookup("./kernel/mm/transparent_hugepage/enabled")?;
    file_hp.write_at(0, b"always [madvise] never\n").await?;

    // Create /sys/devices/system/clocksource/clocksource0/current_clocksource
    sys_root.create("devices", VfsNodeType::Dir).await?;
    sys_root.create("devices/system", VfsNodeType::Dir).await?;
    sys_root.create("devices/system/clocksource", VfsNodeType::Dir).await?;
    sys_root.create("devices/system/clocksource/clocksource0", VfsNodeType::Dir).await?;
    sys_root.create(
        "devices/system/clocksource/clocksource0/current_clocksource",
        VfsNodeType::File,
    ).await?;
    let file_cc = sys_root
        .clone()
        .lookup("devices/system/clocksource/clocksource0/current_clocksource")?;
    file_cc.write_at(0, b"tsc\n").await?;

    // create /sys/devices/system/cpu/online
    sys_root.create("devices/system/cpu", VfsNodeType::Dir).await?;
    sys_root.create("devices/system/cpu/online", VfsNodeType::File).await?;
    let cpu_online = sys_root.clone().lookup("devices/system/cpu/online")?;
    let smp = axconfig::SMP;
    cpu_online.write_at(0, alloc::format!("0-{}", smp - 1).as_bytes()).await?;
    for cpu_id in 0..smp {
        let path = alloc::format!("devices/system/cpu/cpu{}", cpu_id);
        sys_root.create(path.as_str(), VfsNodeType::Dir).await?;
        let path = path + "/online";
        sys_root.create(path.as_str(), VfsNodeType::File).await?;
        let file = sys_root.clone().lookup(path.as_str())?;
        file.write_at(0, b"1").await?;
    }
    Ok(Arc::new(sysfs))
}
