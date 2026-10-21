use std::sync::Mutex;

use serde::Serialize;
use windows::Devices::Radios::{Radio, RadioAccessStatus, RadioKind, RadioState};
use windows::Win32::Foundation::{ERROR_SUCCESS, HANDLE, VARIANT_BOOL};
use windows::Win32::NetworkManagement::WiFi::{
    WLAN_CONNECTION_ATTRIBUTES, WLAN_INTERFACE_INFO, WLAN_INTERFACE_INFO_LIST, WlanCloseHandle,
    WlanEnumInterfaces, WlanFreeMemory, WlanOpenHandle, WlanQueryInterface,
    wlan_intf_opcode_current_connection, wlan_interface_state_connected,
};
use windows::Win32::Networking::NetworkListManager::{
    INetworkListManager, NLM_ENUM_NETWORK_CONNECTED, NetworkListManager,
};
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_DWORD, RegGetValueW, RegSetKeyValueW, RRF_RT_REG_DWORD,
};
use windows::Win32::UI::Accessibility::{STICKYKEYS, STICKYKEYS_FLAGS, SKF_STICKYKEYSON};
use windows::Win32::UI::WindowsAndMessaging::{
    SPIF_UPDATEINIFILE, SPI_GETSTICKYKEYS, SPI_SETSTICKYKEYS, SystemParametersInfoW,
    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
};
use windows::core::{BSTR, Result as WinResult, w};

use crate::commands::util::{
    ComGuard, Variant, block_on, blocking, variant_as_u32, wmi_query, wmi_services,
};

#[derive(Serialize)]
pub struct NetworkState {
    pub connected: bool,
    pub internet: bool,
    pub name: Option<String>,
    pub ssid: Option<String>,
    pub signal: Option<u32>,
}

#[derive(Serialize)]
pub struct TilesState {
    pub wifi: Option<bool>,
    pub bluetooth: Option<bool>,
    pub airplane: bool,
    pub sticky: bool,
    pub saver: bool,
    pub dnd: bool,
}

fn network_state() -> Result<NetworkState, String> {
    let _com = ComGuard::new();

    let manager: INetworkListManager = unsafe {
        CoCreateInstance(&NetworkListManager, None, CLSCTX_ALL)
            .map_err(|e| format!("NetworkListManager: {e}"))?
    };

    let connected = unsafe { manager.IsConnected() }
        .map_err(|e| format!("IsConnected: {e}"))?
        != VARIANT_BOOL(0);
    let internet = unsafe { manager.IsConnectedToInternet() }
        .map_err(|e| format!("IsConnectedToInternet: {e}"))?
        != VARIANT_BOOL(0);

    let mut name = None;
    if connected {
        if let Ok(enum_) = unsafe { manager.GetNetworks(NLM_ENUM_NETWORK_CONNECTED) } {
            let mut fetched = 0u32;
            let mut slots: [Option<windows::Win32::Networking::NetworkListManager::INetwork>; 1] =
                [None];
            let _ = unsafe { enum_.Next(&mut slots, Some(&mut fetched)) };
            if fetched > 0 {
                if let Some(net) = slots[0].take() {
                    name = unsafe { net.GetName() }.ok().map(|s| s.to_string());
                }
            }
        }
    }

    let (ssid, signal) = wifi_ssid_signal().map_or((None, None), |(s, q)| (Some(s), Some(q)));

    Ok(NetworkState {
        connected,
        internet,
        name,
        ssid,
        signal,
    })
}

fn wifi_ssid_signal() -> Option<(String, u32)> {
    let mut negotiated = 0u32;
    let mut handle = HANDLE(std::ptr::null_mut());
    if unsafe { WlanOpenHandle(2, None, &mut negotiated, &mut handle) } != 0 {
        return None;
    }

    let mut result: Option<(String, u32)> = None;
    let mut list: *mut WLAN_INTERFACE_INFO_LIST = std::ptr::null_mut();

    if unsafe { WlanEnumInterfaces(handle, None, &mut list) } == 0 && !list.is_null() {
        let count = unsafe { (*list).dwNumberOfItems };
        let base = unsafe { std::ptr::addr_of!((*list).InterfaceInfo) } as *const WLAN_INTERFACE_INFO;
        for i in 0..count {
            let info = unsafe { &*base.add(i as usize) };
            if info.isState != wlan_interface_state_connected {
                continue;
            }
            let mut size = 0u32;
            let mut data: *mut core::ffi::c_void = std::ptr::null_mut();
            let err = unsafe {
                WlanQueryInterface(
                    handle,
                    &info.InterfaceGuid,
                    wlan_intf_opcode_current_connection,
                    None,
                    &mut size,
                    &mut data,
                    None,
                )
            };
            if err != 0 || data.is_null() {
                continue;
            }
            let conn = unsafe { &*(data as *const WLAN_CONNECTION_ATTRIBUTES) };
            if conn.isState == wlan_interface_state_connected {
                let ssid = &conn.wlanAssociationAttributes.dot11Ssid;
                let len = (ssid.uSSIDLength as usize).min(ssid.ucSSID.len());
                let text = String::from_utf8_lossy(&ssid.ucSSID[..len]).into_owned();
                result = Some((text, conn.wlanAssociationAttributes.wlanSignalQuality));
            }
            unsafe { WlanFreeMemory(data as _) };
            break;
        }
    }

    if !list.is_null() {
        unsafe { WlanFreeMemory(list as _) };
    }
    unsafe { WlanCloseHandle(handle, None) };
    result
}

#[tauri::command]
pub async fn controls_network() -> Result<NetworkState, String> {
    blocking(network_state).await
}

fn all_radios() -> WinResult<Vec<Radio>> {
    let op = Radio::GetRadiosAsync()?;
    let list = block_on(async move { op.await })?;
    let n = list.Size()?;
    let mut out = Vec::with_capacity(n as usize);
    for i in 0..n {
        out.push(list.GetAt(i)?);
    }
    Ok(out)
}

fn radio_apply(radio: &Radio, state: RadioState) -> Result<(), String> {
    // Si ya está en el estado pedido, `SetStateAsync` devuelve
    // 0x8007139F («el grupo o recurso no está en el estado correcto») y no hay
    // nada que hacer: tratarlo como éxito para no sacar un error fantasma.
    if radio.State().ok() == Some(state) {
        return Ok(());
    }
    let op = radio
        .SetStateAsync(state)
        .map_err(|e| format!("Radio: {e}"))?;
    let status = match block_on(async move { op.await }) {
        Ok(status) => status,
        Err(e) if is_wrong_state(&e) => return Ok(()),
        Err(e) => return Err(format!("Radio: {e}")),
    };
    match status {
        RadioAccessStatus::Allowed => Ok(()),
        RadioAccessStatus::DeniedByUser => Err("Permiso denegado por el usuario".to_string()),
        RadioAccessStatus::DeniedBySystem => Err("La radio está bloqueada por el sistema".to_string()),
        _ => Err("No se pudo cambiar el estado de la radio".to_string()),
    }
}

/// HRESULT 0x8007139F (ERROR_INVALID_STATE de WinRT).
fn is_wrong_state(err: &windows::core::Error) -> bool {
    err.code().0 as u32 & 0xFFFF_FFFF == 0x8007_139F
}

fn find_radio(radios: &[Radio], kind: RadioKind) -> Option<Radio> {
    radios.iter().find_map(|r| {
        if matches!(r.Kind(), Ok(k) if k == kind) {
            Some(r.clone())
        } else {
            None
        }
    })
}

fn radio_set_kind(kind: RadioKind, on: bool) -> Result<(), String> {
    let _com = ComGuard::new();
    let radios = all_radios().map_err(|e| format!("Radios: {e}"))?;
    let radio = find_radio(&radios, kind)
        .ok_or_else(|| "Esta radio no está disponible en este equipo".to_string())?;
    radio_apply(&radio, if on { RadioState::On } else { RadioState::Off })
}

static AIRPLANE_SAVED: Mutex<Option<Vec<((String, RadioKind), RadioState)>>> = Mutex::new(None);

fn airplane_set(on: bool) -> Result<(), String> {
    let _com = ComGuard::new();
    let radios = all_radios().map_err(|e| format!("Radios: {e}"))?;
    if on {
        let saved: Vec<((String, RadioKind), RadioState)> = radios
            .iter()
            .filter_map(|r| {
                let name = r.Name().ok()?.to_string();
                let kind = r.Kind().ok()?;
                let state = r.State().ok()?;
                Some(((name, kind), state))
            })
            .collect();
        *AIRPLANE_SAVED.lock().unwrap() = Some(saved);
        for radio in &radios {
            if radio.State().ok() == Some(RadioState::On) {
                radio_apply(radio, RadioState::Off)?;
            }
        }
        Ok(())
    } else {
        let saved = AIRPLANE_SAVED.lock().unwrap().take();
        for radio in &radios {
            let name = radio.Name().ok().map(|h| h.to_string());
            let kind = radio.Kind().ok();
            let target = saved
                .as_ref()
                .and_then(|entries| {
                    entries
                        .iter()
                        .find(|((n, k), _)| Some(n) == name.as_ref() && Some(*k) == kind)
                        .map(|(_, st)| *st)
                })
                .unwrap_or(RadioState::On);
            if radio.State().ok() == Some(target) {
                continue;
            }
            radio_apply(radio, target)?;
        }
        Ok(())
    }
}

fn sticky_get() -> Result<bool, String> {
    let mut sk = STICKYKEYS {
        cbSize: size_of::<STICKYKEYS>() as u32,
        dwFlags: STICKYKEYS_FLAGS(0),
    };
    unsafe {
        SystemParametersInfoW(
            SPI_GETSTICKYKEYS,
            size_of::<STICKYKEYS>() as u32,
            Some(&mut sk as *mut STICKYKEYS as *mut core::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    }
    .map_err(|e| format!("Accesibilidad: {e}"))?;
    Ok(sk.dwFlags.0 & SKF_STICKYKEYSON.0 != 0)
}

fn sticky_set(on: bool) -> Result<(), String> {
    let mut sk = STICKYKEYS {
        cbSize: size_of::<STICKYKEYS>() as u32,
        dwFlags: STICKYKEYS_FLAGS(0),
    };
    unsafe {
        SystemParametersInfoW(
            SPI_GETSTICKYKEYS,
            size_of::<STICKYKEYS>() as u32,
            Some(&mut sk as *mut STICKYKEYS as *mut core::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    }
    .map_err(|e| format!("Accesibilidad: {e}"))?;
    if on {
        sk.dwFlags = STICKYKEYS_FLAGS(sk.dwFlags.0 | SKF_STICKYKEYSON.0);
    } else {
        sk.dwFlags = STICKYKEYS_FLAGS(sk.dwFlags.0 & !SKF_STICKYKEYSON.0);
    }
    unsafe {
        SystemParametersInfoW(
            SPI_SETSTICKYKEYS,
            size_of::<STICKYKEYS>() as u32,
            Some(&mut sk as *mut STICKYKEYS as *mut core::ffi::c_void),
            SPIF_UPDATEINIFILE,
        )
    }
    .map_err(|e| format!("Accesibilidad: {e}"))
}

const SCHEME_SAVER: &str = "a1841308-3541-4fab-bc81-f71556f20b4a";
const SCHEME_BALANCED: &str = "381b4222-f694-41f0-9685-ff5bb260df2e";

fn powercfg(args: &[&str]) -> Result<String, String> {
    use std::os::windows::process::CommandExt;
    // CREATE_NO_WINDOW evita que se abra una consola al invocar powercfg.exe
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = std::process::Command::new("powercfg")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("No se pudo ejecutar powercfg: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    if !out.status.success() {
        let detail = stdout.trim();
        let detail = if detail.is_empty() {
            String::from_utf8_lossy(&out.stderr).trim().to_string()
        } else {
            detail.to_string()
        };
        return Err(format!("powercfg: {detail}"));
    }
    Ok(stdout)
}

fn first_guid(text: &str) -> Option<String> {
    let start = text.find('{')?;
    let end = text[start..].find('}')? + start;
    Some(text[start + 1..end].to_string())
}

fn active_scheme_guid() -> Option<String> {
    powercfg(&["/getactivescheme"])
        .ok()
        .and_then(|out| first_guid(&out))
}

/// Lista todos los planes de energía como pares `(guid, nombre)`.
pub fn powercfg_list() -> Result<Vec<(String, String)>, String> {
    let out = powercfg(&["/list"])?;
    let mut plans = Vec::new();
    for line in out.lines() {
        let Some(start) = line.find('{') else { continue };
        let Some(end_rel) = line[start..].find('}') else { continue };
        let guid = line[start + 1..start + end_rel].to_string();
        // El nombre aparece después de la llave de cierre, separado por «: ».
        let rest = &line[start + end_rel + 1..];
        let name = rest
            .split_once(':')
            .map(|(_, n)| n.trim().to_string())
            .filter(|n| !n.is_empty());
        let Some(name) = name else { continue };
        // Las líneas con "*" marcan el plan activo; el guid basta para saberlo.
        plans.push((guid, name));
    }
    Ok(plans)
}

pub fn active_plan_guid() -> Option<String> {
    active_scheme_guid()
}

pub fn set_active_plan(guid: &str) -> Result<(), String> {
    powercfg(&["/setactive", guid]).map(|_| ())
}

fn saver_active() -> bool {
    active_scheme_guid()
        .is_some_and(|g| g.eq_ignore_ascii_case(SCHEME_SAVER))
}

fn saver_set(on: bool) -> Result<(), String> {
    let target = if on { SCHEME_SAVER } else { SCHEME_BALANCED };
    match powercfg(&["/setactive", target]) {
        Ok(_) => Ok(()),
        Err(err) if !on => Err(err),
        Err(_) => {
            let duplicated = powercfg(&["/duplicatescheme", SCHEME_SAVER])?;
            let new_guid =
                first_guid(&duplicated).ok_or("No se pudo preparar el esquema de ahorro")?;
            powercfg(&["/setactive", &new_guid])?;
            Ok(())
        }
    }
}

fn dnd_get() -> Result<bool, String> {
    let mut data: u32 = 1;
    let mut size = size_of::<u32>() as u32;
    let err = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Notifications\\Settings"),
            w!("NOC_GLOBAL_SETTING_TOASTS_ENABLED"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut data as *mut u32 as *mut core::ffi::c_void),
            Some(&mut size),
        )
    };
    if err != ERROR_SUCCESS {
        return Ok(false);
    }
    Ok(data == 0)
}

fn dnd_set(on: bool) -> Result<(), String> {
    let value: u32 = if on { 0 } else { 1 };
    let err = unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            w!("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Notifications\\Settings"),
            w!("NOC_GLOBAL_SETTING_TOASTS_ENABLED"),
            REG_DWORD.0,
            Some(&value as *const u32 as *const core::ffi::c_void),
            size_of::<u32>() as u32,
        )
    };
    if err != ERROR_SUCCESS {
        return Err(format!("No se pudo cambiar «No molestar» (código {err:?})"));
    }
    Ok(())
}

fn brightness_get() -> Result<Option<u8>, String> {
    let _com = ComGuard::new();
    let svc = wmi_services("root\\wmi")?;
    let Some(obj) = wmi_query(&svc, "SELECT * FROM WmiMonitorBrightness")? else {
        return Ok(None);
    };
    let props = unsafe { obj.Properties_() }.map_err(|e| format!("Brillo: {e}"))?;
    let prop = unsafe { props.Item(&BSTR::from("CurrentBrightness"), 0) }
        .map_err(|e| format!("Brillo: {e}"))?;
    let value = unsafe { prop.Value() }.map_err(|e| format!("Brillo: {e}"))?;
    Ok(unsafe { variant_as_u32(&value) }.map(|v| v.min(100) as u8))
}

fn brightness_set(level: u8) -> Result<(), String> {
    let _com = ComGuard::new();
    let svc = wmi_services("root\\wmi")?;
    let obj = wmi_query(&svc, "SELECT * FROM WmiMonitorBrightnessMethods")?
        .ok_or_else(|| "El brillo no está disponible en este equipo".to_string())?;
    let methods = unsafe { obj.Methods_() }.map_err(|e| format!("Brillo: {e}"))?;
    let method = unsafe { methods.Item(&BSTR::from("WmiSetBrightness"), 0) }
        .map_err(|e| format!("Brillo: {e}"))?;
    let in_params = unsafe { method.InParameters() }.map_err(|e| format!("Brillo: {e}"))?;
    let args = unsafe { in_params.SpawnInstance_(0) }.map_err(|e| format!("Brillo: {e}"))?;
    let props = unsafe { args.Properties_() }.map_err(|e| format!("Brillo: {e}"))?;
    unsafe { props.Item(&BSTR::from("Brightness"), 0) }
        .and_then(|p| unsafe { p.SetValue(Variant::u32(level as u32).get()) })
        .map_err(|e| format!("Brillo: {e}"))?;
    unsafe { props.Item(&BSTR::from("Timeout"), 0) }
        .and_then(|p| unsafe { p.SetValue(Variant::u32(0).get()) })
        .map_err(|e| format!("Brillo: {e}"))?;
    match unsafe { obj.ExecMethod_(&BSTR::from("WmiSetBrightness"), &args, 0, None) } {
        Ok(_) => {}
        // La metadata de wbemdisp declara un [out,retval] que la interfaz real
        // no rellena: devuelve S_OK con el puntero a null y windows-rs lo
        // traduce como Err(S_OK). El método sí se aplica (verificado con
        // brightness_get), así que ese error se considera éxito.
        Err(e) if e.code().is_ok() => {}
        Err(e) => return Err(format!("Brillo: {e}")),
    }
    Ok(())
}

fn tiles_state() -> Result<TilesState, String> {
    let _com = ComGuard::new();
    let radios = all_radios().map_err(|e| format!("Radios: {e}"))?;
    let state_of = |kind: RadioKind| -> Option<bool> {
        radios.iter().find_map(|r| {
            if matches!(r.Kind(), Ok(k) if k == kind) {
                r.State().ok().map(|s| s == RadioState::On)
            } else {
                None
            }
        })
    };
    let wifi = state_of(RadioKind::WiFi);
    let bluetooth = state_of(RadioKind::Bluetooth);
    let any_radio = !radios.is_empty();
    let any_on = radios
        .iter()
        .filter_map(|r| r.State().ok())
        .any(|s| s == RadioState::On);
    Ok(TilesState {
        wifi,
        bluetooth,
        airplane: any_radio && !any_on,
        sticky: sticky_get()?,
        saver: saver_active(),
        dnd: dnd_get()?,
    })
}

#[tauri::command]
pub async fn controls_tiles_state() -> Result<TilesState, String> {
    blocking(tiles_state).await
}

#[tauri::command]
pub async fn controls_wifi_set(on: bool) -> Result<(), String> {
    blocking(move || radio_set_kind(RadioKind::WiFi, on)).await
}

#[tauri::command]
pub async fn controls_bluetooth_set(on: bool) -> Result<(), String> {
    blocking(move || radio_set_kind(RadioKind::Bluetooth, on)).await
}

#[tauri::command]
pub async fn controls_airplane_set(on: bool) -> Result<(), String> {
    blocking(move || airplane_set(on)).await
}

#[tauri::command]
pub async fn controls_sticky_set(on: bool) -> Result<(), String> {
    blocking(move || sticky_set(on)).await
}

#[tauri::command]
pub async fn controls_saver_set(on: bool) -> Result<(), String> {
    blocking(move || saver_set(on)).await
}

#[tauri::command]
pub async fn controls_dnd_set(on: bool) -> Result<(), String> {
    blocking(move || dnd_set(on)).await
}

#[tauri::command]
pub async fn controls_brightness_get() -> Result<Option<u8>, String> {
    blocking(brightness_get).await
}

#[tauri::command]
pub async fn controls_brightness_set(level: u8) -> Result<(), String> {
    blocking(move || brightness_set(level)).await
}
