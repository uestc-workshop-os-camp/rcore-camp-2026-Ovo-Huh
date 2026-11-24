//! Process management syscalls
use core;
use crate::task::get_syscall_times;
use crate::config::PAGE_SIZE;
use crate::mm::{translated_byte_buffer, MapPermission, PageTable, VirtAddr};
use crate::task::{change_program_brk, current_user_token, exit_current_and_run_next, suspend_current_and_run_next, TASK_MANAGER};
use crate::timer::get_time_us;
#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let sec = get_time_us() / 1_000_000;
    let usec = get_time_us() % 1_000_000;
    let token = current_user_token();
    let ptr = ts as *const u8;
    let len = core::mem::size_of::<TimeVal>();
    let buffers = translated_byte_buffer(token, ptr, len);
    let timeval = TimeVal{sec,usec};
    let mut src = &timeval as *const TimeVal as *const u8;
    for buffer in buffers{
        unsafe{
            core::ptr::copy_nonoverlapping(src, buffer.as_mut_ptr(), buffer.len());
            src = src.add(buffer.len());
        }
    }
    0
}

/// TODO: Finish sys_trace to pass testcases
/// HINT: You might reimplement it with virtual memory management.
pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    trace!("kernel: sys_trace");
    let token = current_user_token();
    let page_table = PageTable::from_token(token);
    let va = VirtAddr::from(id);
    if id >= (1usize << 38) {
        return -1;
    }
    let vpn = va.floor();
    let offset = va.page_offset();
    match trace_request{
        0 => {
            let ptee = page_table.translate(vpn);
            if ptee.is_none(){return -1};
            let pte = ptee.unwrap();
            if !pte.is_valid() || !pte.readable(){
               return -1;
            }
            let ppn = pte.ppn();
            let bytes = ppn.get_bytes_array();
            let value = bytes[offset];
            value as isize 
        }
        1 => {
            let ptee = page_table.translate(vpn);
            if ptee.is_none(){return -1};
            let pte = ptee.unwrap();
            if !pte.is_valid() || !pte.writable(){
               return -1;
            }
            let ppn = pte.ppn();
            let bytes = ppn.get_bytes_array();
            bytes[offset] = data as u8; 
            0
        }
        2 => get_syscall_times(id),
        _ => -1
    }
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(start: usize, len: usize, prot: usize) -> isize {
    trace!("kernel: sys_mmap");
    let permission = match prot {
        1 => MapPermission::R | MapPermission::U,
        2 => MapPermission::W | MapPermission::U,
        3 => MapPermission::R | MapPermission::W | MapPermission::U,
        4 => MapPermission::X | MapPermission::U,
        5 => MapPermission::R | MapPermission::X | MapPermission::U,
        6 => MapPermission::W | MapPermission::X | MapPermission::U,
        7 => MapPermission::R | MapPermission::W | MapPermission::X | MapPermission::U,
        _ => return -1,
    };

    if start % PAGE_SIZE != 0 || len == 0 {return -1;}

    let end = match start.checked_add(len) {
        Some(end) => end,
        None => return -1,
    };

    let start_va = VirtAddr::from(start);
    let end_va = VirtAddr::from(end);
    if end > (1usize << 38) || TASK_MANAGER.is_current_area_overlap(start_va, end_va) {return -1;}
    TASK_MANAGER.insert_current_framed_area(start_va, end_va, permission);
    0
}
// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sys_munmap");
    let start_va = VirtAddr::from(start);
    if start % PAGE_SIZE != 0 || len == 0 || len % PAGE_SIZE != 0{return -1}
    if TASK_MANAGER.remove_current_area(start_va) {
        0
    } else {
        -1
    }
}
/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
