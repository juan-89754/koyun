use std::sync::Mutex;
use std::time::Instant;

use serde::Serialize;
use windows::Win32::Foundation::{ERROR_SUCCESS, FILETIME};
use windows::Win32::NetworkManagement::IpHelper::{FreeMibTable, GetIfTable2, MIB_IF_ROW2, MIB_IF_TABLE2};
use windows::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives,
};
use windows::Win32::System::Performance::{
    PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY, PdhAddEnglishCounterW,
    PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW, PdhOpenQueryW,
};
use windows::Win32::System::SystemInformation::{
    GlobalMemoryStatusEx, GetTickCount64, MEMORYSTATUSEX,
};
use windows::Win32::System::Threading::GetSystemTimes;
use windows::core::{BSTR, PCWSTR, w};

use crate::commands::util::{
    ComGuard, blocking, variant_as_string, variant_as_u32, wmi_query, wmi_services,
};

const DRIVE_FIXED: u32 = 3;
const IF_TYPE_LOOPBACK: u32 = 24;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn filetime_ticks(ft: FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64
}

static CPU_LAST: Mutex<Option<(u64, u64, u64)>> = Mutex::new(None);
static NET_LAST: Mutex<Option<(Instant, u64, u64)>> = Mutex::new(None);

fn cpu_usage() -> Option<f32> {
    let mut idle = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) }.ok()?;
    let now = (
        filetime_ticks(idle),
        filetime_ticks(kernel),
        filetime_ticks(user),
    );
    let mut last = CPU_LAST.lock().unwrap();
    let usage = match *last {
        Some((prev_idle, prev_kernel, prev_user)) => {
            let d_idle = now.0.saturating_sub(prev_idle) as f64;
            let d_total = (now.1 + now.2).saturating_sub(prev_kernel + prev_user) as f64;
            if d_total <= 0.0 {
                None
            } else {
                Some((((1.0 - d_idle / d_total) * 100.0).clamp(0.0, 100.0)) as f32)
            }
        }
        None => None,
    };
    *last = Some(now);
    usage
}

fn if_octets() -> Option<(u64, u64)> {
    let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
    if unsafe { GetIfTable2(&mut table) } != ERROR_SUCCESS || table.is_null() {
        return None;
    }
    let mut in_total = 0u64;
    let mut out_total = 0u64;
    unsafe {
        let n = (*table).NumEntries as usize;
        let base = std::ptr::addr_of!((*table).Table) as *const MIB_IF_ROW2;
        for i in 0..n {
            let row = &*base.add(i);
            if row.Type == IF_TYPE_LOOPBACK {
                continue;
            }
            in_total = in_total.saturating_add(row.InOctets);
            out_total = out_total.saturating_add(row.OutOctets);
        }
        FreeMibTable(table as *const core::ffi::c_void);
    }
    Some((in_total, out_total))
}

fn net_rates() -> (Option<f64>, Option<f64>) {
    let Some((in_total, out_total)) = if_octets() else {
        return (None, None);
    };
    let mut last = NET_LAST.lock().unwrap();
    match *last {
        Some((prev_t, prev_in, prev_out)) => {
            let dt = prev_t.elapsed().as_secs_f64();
            *last = Some((Instant::now(), in_total, out_total));
            if dt <= 0.0 {
                return (None, None);
            }
            let down = (in_total.saturating_sub(prev_in) as f64) * 8.0 / dt;
            let up = (out_total.saturating_sub(prev_out) as f64) * 8.0 / dt;
            (Some(down), Some(up))
        }
        None => {
            *last = Some((Instant::now(), in_total, out_total));
            (None, None)
        }
    }
}

#[derive(Serialize)]
pub struct DiskInfo {
    pub name: String,
    pub total: u64,
    pub free: u64,
}

fn disks() -> Vec<DiskInfo> {
    let mask = unsafe { GetLogicalDrives() };
    let mut out = Vec::new();
    for i in 0..26u32 {
        if mask & (1 << i) == 0 {
            continue;
        }
        let letter = (b'A' + i as u8) as char;
        let root = wide(&format!("{letter}:\\"));
        let root_ptr = PCWSTR(root.as_ptr());
        if unsafe { GetDriveTypeW(root_ptr) } != DRIVE_FIXED {
            continue;
        }
        let mut free = 0u64;
        let mut total = 0u64;
        let ok = unsafe {
            GetDiskFreeSpaceExW(root_ptr, None, Some(&mut total), Some(&mut free))
        };
        if ok.is_ok() {
            out.push(DiskInfo {
                name: letter.to_string(),
                total,
                free,
            });
        }
    }
    out
}

struct GpuQuery {
    query: PDH_HQUERY,
    counter: PDH_HCOUNTER,
    primed: bool,
}

unsafe impl Send for GpuQuery {}

static GPU: Mutex<Option<GpuQuery>> = Mutex::new(None);

fn gpu_usage() -> Option<f32> {
    let mut guard = GPU.lock().unwrap();
    if guard.is_none() {
        let mut query = PDH_HQUERY::default();
        let mut counter = PDH_HCOUNTER::default();
        if unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &mut query) } != 0 {
            return None;
        }
        let rc = unsafe {
            PdhAddEnglishCounterW(
                query,
                w!(r"\GPU Engine(*)\Utilization Percentage"),
                0,
                &mut counter,
            )
        };
        if rc != 0 {
            unsafe { PdhCloseQuery(query) };
            return None;
        }
        unsafe { PdhCollectQueryData(query) };
        *guard = Some(GpuQuery {
            query,
            counter,
            primed: false,
        });
        return None;
    }

    let gpu = guard.as_mut().unwrap();
    if unsafe { PdhCollectQueryData(gpu.query) } != 0 {
        return None;
    }
    if !gpu.primed {
        gpu.primed = true;
        return None;
    }

    let mut size = 0u32;
    let mut count = 0u32;
    unsafe {
        PdhGetFormattedCounterArrayW(gpu.counter, PDH_FMT_DOUBLE, &mut size, &mut count, None)
    };
    if count == 0 || size == 0 {
        return None;
    }

    // `size` está en bytes e incluye las cadenas de nombres de instancia, que PDH
    // escribe dentro del propio búfer. Se reserva ese tamaño exacto en bytes y se
    // usa `Vec<u64>` para garantizar la alineación de PDH_FMT_COUNTERVALUE_ITEM_W.
    let words = (size as usize).div_ceil(std::mem::size_of::<u64>());
    let mut buffer = vec![0u64; words];
    let mut buffer_size = (words * std::mem::size_of::<u64>()) as u32;
    let rc = unsafe {
        PdhGetFormattedCounterArrayW(
            gpu.counter,
            PDH_FMT_DOUBLE,
            &mut buffer_size,
            &mut count,
            Some(buffer.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W),
        )
    };
    if rc != 0 || count == 0 || (count as usize) * std::mem::size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>() > buffer_size as usize {
        return None;
    }
    let items = unsafe {
        std::slice::from_raw_parts(
            buffer.as_ptr() as *const PDH_FMT_COUNTERVALUE_ITEM_W,
            count as usize,
        )
    };
    let max = items
        .iter()
        .map(|it| unsafe { it.FmtValue.Anonymous.doubleValue })
        .fold(0.0f64, f64::max);
    Some(max.clamp(0.0, 100.0) as f32)
}

fn wmi_prop_string(obj: &windows::Win32::System::Wmi::ISWbemObject, name: &str) -> Option<String> {
    let props = unsafe { obj.Properties_() }.ok()?;
    let prop = unsafe { props.Item(&BSTR::from(name), 0) }.ok()?;
    let value = unsafe { prop.Value() }.ok()?;
    unsafe { variant_as_string(value) }
}

fn wmi_prop_u32(obj: &windows::Win32::System::Wmi::ISWbemObject, name: &str) -> Option<u64> {
    let props = unsafe { obj.Properties_() }.ok()?;
    let prop = unsafe { props.Item(&BSTR::from(name), 0) }.ok()?;
    let value = unsafe { prop.Value() }.ok()?;
    unsafe { variant_as_u32(&value) }.map(|v| v as u64)
}

fn gpu_info() -> (Option<String>, Option<u64>) {
    let _com = ComGuard::new();
    let Ok(svc) = wmi_services("root\\cimv2") else {
        return (None, None);
    };
    let Ok(Some(obj)) = wmi_query(&svc, "SELECT * FROM Win32_VideoController") else {
        return (None, None);
    };
    // `AdapterRAM` es u32: WMI devuelve 0xFFFFFFFF cuando la VRAM supera los 4 GB.
    let vram = wmi_prop_u32(&obj, "AdapterRAM").filter(|v| *v != u32::MAX as u64);
    (wmi_prop_string(&obj, "Name"), vram)
}

/// La consulta WMI de la GPU no cambia nunca en caliente: hacerla en cada
/// instantánea (1,5 s) solo costaba decenas de ms de COM/WMI sin necesidad.
static GPU_INFO: Mutex<Option<(Option<String>, Option<u64>)>> = Mutex::new(None);

fn gpu_info_cached() -> (Option<String>, Option<u64>) {
    let mut guard = GPU_INFO.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(info) = guard.as_ref() {
        return info.clone();
    }
    let info = gpu_info();
    // No cachear los fallos: deja que se reintente en la siguiente instantánea.
    if info.0.is_some() {
        *guard = Some(info.clone());
    }
    info
}

#[derive(Serialize)]
pub struct StatsSnapshot {
    pub cpu: Option<f32>,
    pub ram_used: u64,
    pub ram_total: u64,
    pub gpu_name: Option<String>,
    pub gpu_vram: Option<u64>,
    pub gpu_usage: Option<f32>,
    pub disks: Vec<DiskInfo>,
    pub uptime_ms: u64,
    pub net_down_bps: Option<f64>,
    pub net_up_bps: Option<f64>,
}

#[tauri::command]
pub async fn stats_snapshot() -> Result<StatsSnapshot, String> {
    blocking(snapshot_inner).await
}

fn snapshot_inner() -> Result<StatsSnapshot, String> {
    let mut mem = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        dwMemoryLoad: 0,
        ullTotalPhys: 0,
        ullAvailPhys: 0,
        ullTotalPageFile: 0,
        ullAvailPageFile: 0,
        ullTotalVirtual: 0,
        ullAvailVirtual: 0,
        ullAvailExtendedVirtual: 0,
    };
    let (ram_total, ram_used) = if unsafe { GlobalMemoryStatusEx(&mut mem) }.is_ok() {
        (
            mem.ullTotalPhys,
            mem.ullTotalPhys.saturating_sub(mem.ullAvailPhys),
        )
    } else {
        (0, 0)
    };

    let (net_down_bps, net_up_bps) = net_rates();
    let (gpu_name, gpu_vram) = gpu_info_cached();

    Ok(StatsSnapshot {
        cpu: cpu_usage(),
        ram_used,
        ram_total,
        gpu_name,
        gpu_vram,
        gpu_usage: gpu_usage(),
        disks: disks(),
        uptime_ms: unsafe { GetTickCount64() },
        net_down_bps,
        net_up_bps,
    })
}
