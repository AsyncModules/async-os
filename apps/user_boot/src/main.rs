#![no_std]
#![no_main]

use alloc::{string::{String, ToString}, vec::Vec, vec};

extern crate async_std;
extern crate trampoline;

#[async_std::async_main]
async fn main() -> isize {
    async_std::println!("user_boot");
    // 初始化文件系统
    trampoline::fs_init().await;
    // for testcase in BUSYBOX_TESTCASES {
    for testcase in TESTCASES {
        let task = trampoline::init_user(get_args(testcase.as_bytes()), &get_envs().await)
        .await
        .unwrap();
        trampoline::wait(&task).await;
        async_std::println!("task count {}", alloc::sync::Arc::strong_count(&task));
    }
    0
}

/// Now the environment variables are hard coded, we need to read the file "/etc/environment" to get the environment variables
pub async fn get_envs() -> Vec<String> {
    // Const string for environment variables
    let mut envs:Vec<String> = vec![
        "SHLVL=1".into(),
        "PWD=/".into(),
        "GCC_EXEC_PREFIX=/riscv64-linux-musl-native/bin/../lib/gcc/".into(),
        "COLLECT_GCC=./riscv64-linux-musl-native/bin/riscv64-linux-musl-gcc".into(),
        "COLLECT_LTO_WRAPPER=/riscv64-linux-musl-native/bin/../libexec/gcc/riscv64-linux-musl/11.2.1/lto-wrapper".into(),
        "COLLECT_GCC_OPTIONS='-march=rv64gc' '-mabi=lp64d' '-march=rv64imafdc' '-dumpdir' 'a.'".into(),
        "LIBRARY_PATH=/lib/".into(),
        "LD_LIBRARY_PATH=/lib/".into(),
        "LD_DEBUG=files".into(),
    ];
    // read the file "/etc/environment"
    // if exist, then append the content to envs
    // else set the environment variable to default value
    if let Some(environment_vars) = async_std::fs::read_to_string("/etc/environment").await.ok() {
        envs.push(environment_vars);
    } else {
        envs.push("PATH=/usr/sbin:/usr/bin:/sbin:/bin".into());
    }
    envs
}

#[allow(unused)]
/// 分割命令行参数
fn get_args(command_line: &[u8]) -> Vec<String> {
    let mut args = Vec::new();
    // 需要判断是否存在引号，如busybox_cmd.txt的第一条echo指令便有引号
    // 若有引号时，不能把引号加进去，同时要注意引号内的空格不算是分割的标志
    let mut in_quote = false;
    let mut arg_start = 0; // 一个新的参数的开始位置
    for pos in 0..command_line.len() {
        if command_line[pos] == b'\"' {
            in_quote = !in_quote;
        }
        if command_line[pos] == b' ' && !in_quote {
            // 代表要进行分割
            // 首先要防止是否有空串
            if arg_start != pos {
                args.push(
                    core::str::from_utf8(&command_line[arg_start..pos])
                        .unwrap()
                        .to_string(),
                );
            }
            arg_start = pos + 1;
        }
    }
    // 最后一个参数
    if arg_start != command_line.len() {
        args.push(
            core::str::from_utf8(&command_line[arg_start..])
                .unwrap()
                .to_string(),
        );
    }
    args
}

#[allow(dead_code)]
const BUSYBOX_TESTCASES: &[&str] = &[
    "busybox sh busybox_testcode.sh",
    "busybox sh lua_testcode.sh",
    "libctest_testcode.sh",
];

#[allow(dead_code)]
const TESTCASES: &[&str] = &[
    // "hello_world",
    "pipetest",
];