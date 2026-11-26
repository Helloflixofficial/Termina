//! Opts the process out of Windows power/timer throttling and requests High GPU scheduling
//! priority so background/laptop power management does not throttle emulation or Vulkan work.

/// Applies per-process Windows scheduling and power-throttling boosts; no-op on other OSes.
#[cfg(windows)]
pub fn apply() {
    windows::opt_out_power_throttling();
    windows::set_high_gpu_priority();
}

#[cfg(not(windows))]
pub fn apply() {}

#[cfg(windows)]
mod windows {
    use core::ffi::c_void;
    use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, PROCESS_POWER_THROTTLING_CURRENT_VERSION,
        PROCESS_POWER_THROTTLING_EXECUTION_SPEED, PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION,
        PROCESS_POWER_THROTTLING_STATE, ProcessPowerThrottling, SetProcessInformation,
    };

    const GPU_PRIORITY_HIGH: i32 = 2;

    pub(super) fn opt_out_power_throttling() {
        let state = PROCESS_POWER_THROTTLING_STATE {
            Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
            ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED
                | PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION,
            StateMask: 0,
        };
        // SAFETY: `state` is a valid initialized `PROCESS_POWER_THROTTLING_STATE` and `GetCurrentProcess()` returns the pseudo-handle for this process.
        let ok = unsafe {
            SetProcessInformation(
                GetCurrentProcess(),
                ProcessPowerThrottling,
                (&raw const state).cast::<c_void>(),
                core::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
            )
        };
        if ok == 0 {
            tracing::debug!("SetProcessInformation(ProcessPowerThrottling) was refused");
        }
    }

    pub(super) fn set_high_gpu_priority() {
        let gdi32: [u16; 10] = [
            b'g' as u16,
            b'd' as u16,
            b'i' as u16,
            b'3' as u16,
            b'2' as u16,
            b'.' as u16,
            b'd' as u16,
            b'l' as u16,
            b'l' as u16,
            0,
        ];
        // SAFETY: `gdi32` is a null-terminated UTF-16 system DLL name.
        let module = unsafe { LoadLibraryW(gdi32.as_ptr()) };
        if module.is_null() {
            return;
        }
        // SAFETY: `module` is a valid loaded HMODULE and the symbol name is null-terminated ASCII.
        let Some(proc_addr) = (unsafe {
            GetProcAddress(
                module,
                c"D3DKMTSetProcessSchedulingPriorityClass".as_ptr().cast(),
            )
        }) else {
            return;
        };
        type SetGpuPriorityFn = unsafe extern "system" fn(*mut c_void, i32) -> i32;
        // SAFETY: `D3DKMTSetProcessSchedulingPriorityClass` has signature `(HANDLE, D3DKMT_SCHEDULINGPRIORITYCLASS) -> NTSTATUS`.
        let set_priority: SetGpuPriorityFn = unsafe { core::mem::transmute(proc_addr) };
        // SAFETY: `GetCurrentProcess()` is a valid process handle and `GPU_PRIORITY_HIGH` (2) is `D3DKMT_SCHEDULINGPRIORITYCLASS_HIGH`.
        let status = unsafe { set_priority(GetCurrentProcess(), GPU_PRIORITY_HIGH) };
        if status == 0 {
            tracing::info!("GPU scheduling priority set to High");
        } else {
            tracing::debug!(status, "D3DKMTSetProcessSchedulingPriorityClass refused");
        }
    }
}
