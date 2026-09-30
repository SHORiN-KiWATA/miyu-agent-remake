//! Windows 上的生命线（`lifeline.rs`）：核心自己进一个「关了就全结束」的作业对象。
//!
//! 这是放开 `unsafe` 的几个模块之一（施工 7-8）：每个 `unsafe` 块前面写着为什么安全。

#![allow(unsafe_code, reason = "调 Windows 的系统接口（施工 7-8）")]

use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::ptr;
use std::sync::OnceLock;

use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};
use windows_sys::Win32::System::Threading::GetCurrentProcess;

/// 建一个作业对象，设成最后一个句柄关了就结束里面的全部，把这个进程放进去。句柄一直拿到进程结束：进程没了，系统关掉它，
/// 作业里的都结束。只做一次，再调直接交回上一次的结果。
pub(super) fn bind_children() -> io::Result<()> {
    static JOB: OnceLock<Result<OwnedHandle, String>> = OnceLock::new();
    JOB.get_or_init(|| bind().map_err(|error| error.to_string()))
        .as_ref()
        .map(|_| ())
        .map_err(|error| io::Error::other(error.clone()))
}

fn bind() -> io::Result<OwnedHandle> {
    // SAFETY: 没有安全属性、没有名字，传空；交回的是新建的句柄或者空。
    let raw = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
    if raw.is_null() {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `raw` 是刚建成的作业对象的句柄，只在这里接管，丢掉时关一次。
    let job = unsafe { OwnedHandle::from_raw_handle(raw) };
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    let size = u32::try_from(std::mem::size_of_val(&limits)).map_err(io::Error::other)?;
    // SAFETY: `limits` 在调用期间有效，大小照实给；系统只读它。
    let set = unsafe {
        SetInformationJobObject(
            job.as_raw_handle(),
            JobObjectExtendedLimitInformation,
            (&raw const limits).cast(),
            size,
        )
    };
    if set == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `GetCurrentProcess` 交回的是这个进程的伪句柄，不用关；作业的句柄上面拿着，有效。
    let assigned = unsafe { AssignProcessToJobObject(job.as_raw_handle(), GetCurrentProcess()) };
    if assigned == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(job)
}
