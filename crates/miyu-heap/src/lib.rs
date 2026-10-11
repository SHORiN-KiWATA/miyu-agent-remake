//! 进程的堆（`docs/designs/23-性能预算.md` F3，施工 V-2 再补）：核心用系统的分配器；glibc 上调两个参数，空闲时把空着的还给
//! 系统。
//!
//! glibc 释放的内存多半留在自己手里，不还给系统：核心空闲时模型目录读完剩下的、关掉的大会话放下的，进程的内存都不回落（V-2 中
//! 量过：一万条事件的会话关掉以后核心还占 48 MB，空闲时 27 MB）。别的平台（macOS、Windows、musl）的分配器自己会还，这里
//! 什么都不做。

/// glibc 上 arena 最多几个。默认是核数的 8 倍，多开的只多攒碎片；读日志分块在几个线程里一起解（`miyu-store` 的 `MOST`，8 个），
/// 少于它就抢锁：限到 2 个时重开一万条事件的会话，读日志从约 10 ms 慢到 16 ms（施工 V-2 再补量过）。
pub const ARENAS: i32 = 8;

/// 多大的一块直接向系统要、放掉当场还：128 KiB（glibc 起初的默认）。写明了 glibc 就不再自己往上调：默认放掉一块大的以后把门槛
/// 抬到它那么大，之后每一轮拼的请求（几百 KB）都落进堆里，和长住的穿插着放，空出来的碎成小片、还不回去（施工 V-2 再补量过：
/// 大会话退下以后核心匿名内存比空闲时多 8.8 MB，写明以后多 3.9 MB；重开大会话、说一句以后 75 MB 降到 49 MB）。
pub const MMAP_FROM: i32 = 128 * 1024;

/// 起来时、还没起别的线程时调一次：glibc 上 arena 限到 [`ARENAS`] 个，大块的门槛定在 [`MMAP_FROM`]。交回两样都设成了没有；
/// 别的平台什么都不做，交回假。
pub fn tune() -> bool {
    imp::tune(ARENAS, MMAP_FROM)
}

/// 把堆里空着的还给系统：glibc 上调 `malloc_trim(0)`。交回还了没有；别的平台什么都不做，交回假。要走一遍堆，在阻塞线程里调。
pub fn trim() -> bool {
    imp::trim()
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
mod imp {
    use std::os::raw::c_int;

    /// glibc 的 `M_ARENA_MAX`（`malloc.h`）。
    const M_ARENA_MAX: c_int = -8;
    /// glibc 的 `M_MMAP_THRESHOLD`（`malloc.h`）。
    const M_MMAP_THRESHOLD: c_int = -3;

    // 声明 glibc 的两个函数：声明本身不调用，调用的两处各自写明为什么安全。
    #[allow(unsafe_code, reason = "声明 glibc 的 mallopt、malloc_trim")]
    unsafe extern "C" {
        fn mallopt(param: c_int, value: c_int) -> c_int;
        fn malloc_trim(pad: usize) -> c_int;
    }

    pub(super) fn tune(arenas: i32, mmap_from: i32) -> bool {
        set(M_ARENA_MAX, arenas) && set(M_MMAP_THRESHOLD, mmap_from)
    }

    /// 设一个分配器参数，交回设成了没有。
    fn set(param: c_int, value: c_int) -> bool {
        // SAFETY: `mallopt` 只改 glibc 分配器自己的参数，不读写调的一方的内存；参数是 glibc 认得的常量和一个正数。
        #[allow(unsafe_code, reason = "调 glibc 的 mallopt，见上一行")]
        let done = unsafe { mallopt(param, value) };
        done == 1
    }

    pub(super) fn trim() -> bool {
        // SAFETY: `malloc_trim` 只把分配器手里空着的页还给系统，已经分出去的不动；glibc 自己加锁，哪个线程调都行。
        #[allow(unsafe_code, reason = "调 glibc 的 malloc_trim，见上一行")]
        let released = unsafe { malloc_trim(0) };
        released == 1
    }
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
mod imp {
    pub(super) fn tune(_arenas: i32, _mmap_from: i32) -> bool {
        false
    }

    pub(super) fn trim() -> bool {
        false
    }
}

#[cfg(test)]
mod tests;
