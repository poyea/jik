// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

use std::ffi::c_void;

use windows::Win32::System::Threading::{
    GetCurrentProcess, IDLE_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS,
    PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
    PROCESS_POWER_THROTTLING_STATE, ProcessPowerThrottling, SetPriorityClass,
    SetProcessInformation, SetProcessWorkingSetSize,
};

/// Background mode is the same as "Efficiency mode" in Task Manager: EcoQoS and idle priority.
/// When it starts, it also releases the unused memory pages of the process.
pub fn set_background(background: bool) {
    unsafe {
        let process = GetCurrentProcess();
        let state = PROCESS_POWER_THROTTLING_STATE {
            Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
            ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
            StateMask: if background {
                PROCESS_POWER_THROTTLING_EXECUTION_SPEED
            } else {
                0
            },
        };
        let _ = SetProcessInformation(
            process,
            ProcessPowerThrottling,
            &state as *const _ as *const c_void,
            size_of_val(&state) as u32,
        );
        let class = if background {
            IDLE_PRIORITY_CLASS
        } else {
            NORMAL_PRIORITY_CLASS
        };
        let _ = SetPriorityClass(process, class);
        if background {
            let _ = SetProcessWorkingSetSize(process, usize::MAX, usize::MAX);
        }
    }
}
