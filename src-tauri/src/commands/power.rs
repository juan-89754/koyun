use windows::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, LUID};
use windows::Win32::Security::{
    AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED,
    SE_SHUTDOWN_NAME, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows::Win32::System::Power::SetSuspendState;
use windows::Win32::System::Shutdown::{
    ExitWindowsEx, LockWorkStation, EWX_REBOOT, EWX_SHUTDOWN, SHTDN_REASON_FLAG_PLANNED,
    SHTDN_REASON_MAJOR_APPLICATION, SHTDN_REASON_MINOR_OTHER, SHUTDOWN_REASON,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::core::PCWSTR;

fn shutdown_reason() -> SHUTDOWN_REASON {
    SHTDN_REASON_MAJOR_APPLICATION | SHTDN_REASON_MINOR_OTHER | SHTDN_REASON_FLAG_PLANNED
}

fn enable_shutdown_privilege() -> Result<(), String> {
    unsafe {
        let mut token = HANDLE(std::ptr::null_mut());
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token,
        )
        .map_err(|e| format!("OpenProcessToken: {e}"))?;

        let mut luid = LUID::default();
        if let Err(e) = LookupPrivilegeValueW(PCWSTR::null(), SE_SHUTDOWN_NAME, &mut luid) {
            let _ = CloseHandle(token);
            return Err(format!("LookupPrivilegeValueW: {e}"));
        }

        let tp = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES {
                Luid: luid,
                Attributes: SE_PRIVILEGE_ENABLED,
            }],
        };

        // Limpia el último error del hilo: AdjustTokenPrivileges lo reutiliza
        windows::Win32::Foundation::SetLastError(windows::Win32::Foundation::ERROR_SUCCESS);
        let adjust = AdjustTokenPrivileges(token, false, Some(&tp), 0, None, None);
        let err = GetLastError();
        let _ = CloseHandle(token);

        adjust.map_err(|e| format!("AdjustTokenPrivileges: {e}"))?;
        if err != windows::Win32::Foundation::ERROR_SUCCESS {
            return Err(format!("SeShutdownPrivilege no concedido (error {0})", err.0));
        }
        Ok(())
    }
}

#[tauri::command]
pub async fn power_lock() -> Result<(), String> {
    unsafe { LockWorkStation() }.map_err(|e| format!("LockWorkStation falló: {e}"))
}

#[tauri::command]
pub async fn power_suspend() -> Result<(), String> {
    // bhibernate=false => suspender (sleep); no exige hiberfil
    if unsafe { SetSuspendState(false, false, false) } {
        Ok(())
    } else {
        let err = unsafe { GetLastError() };
        Err(format!("SetSuspendState falló (error {0})", err.0))
    }
}

#[tauri::command]
pub async fn power_restart() -> Result<(), String> {
    enable_shutdown_privilege()?;
    unsafe { ExitWindowsEx(EWX_REBOOT, shutdown_reason()) }
        .map_err(|e| format!("ExitWindowsEx(REBOOT) falló: {e}"))
}

#[tauri::command]
pub async fn power_shutdown() -> Result<(), String> {
    enable_shutdown_privilege()?;
    unsafe { ExitWindowsEx(EWX_SHUTDOWN, shutdown_reason()) }
        .map_err(|e| format!("ExitWindowsEx(SHUTDOWN) falló: {e}"))
}
