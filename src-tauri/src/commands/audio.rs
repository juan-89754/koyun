use serde::Serialize;
use windows::core::{GUID, IUnknown, Interface, PCWSTR};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    IMMDevice, IMMDeviceCollection, IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
    eCommunications, eConsole, eMultimedia, eRender,
};
use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PROPVARIANT};
use windows::Win32::System::Com::{
    CLSCTX_ALL, CoCreateInstance, CoTaskMemFree, STGM_READ,
};

use crate::commands::util::{ComGuard, blocking, propvariant_wstring};

#[derive(Serialize)]
pub struct VolumeState {
    pub level: f32,
    pub muted: bool,
}

#[derive(Serialize, Clone)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub default: bool,
}

fn with_audio_endpoint<T>(f: impl FnOnce(&IAudioEndpointVolume) -> windows::core::Result<T>) -> windows::core::Result<T> {
    let _com = ComGuard::new();
    let enumerator: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };
    let device = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eConsole)? };
    let ep: IAudioEndpointVolume = unsafe { device.Activate(CLSCTX_ALL, None)? };
    f(&ep)
}

#[tauri::command]
pub async fn controls_volume_get() -> Result<VolumeState, String> {
    blocking(|| {
        let (level, muted) = with_audio_endpoint(|ep| unsafe {
            let level = ep.GetMasterVolumeLevelScalar()?;
            let muted = ep.GetMute()?.as_bool();
            Ok((level, muted))
        })
        .map_err(|e| format!("Audio: {e}"))?;
        Ok(VolumeState { level, muted })
    })
    .await
}

#[tauri::command]
pub async fn controls_volume_set(level: f32) -> Result<(), String> {
    let level = level.clamp(0.0, 1.0);
    blocking(move || {
        with_audio_endpoint(|ep| unsafe { ep.SetMasterVolumeLevelScalar(level, std::ptr::null()) })
            .map_err(|e| format!("Volumen: {e}"))
    })
    .await
}

#[tauri::command]
pub async fn controls_volume_set_mute(muted: bool) -> Result<(), String> {
    blocking(move || {
        with_audio_endpoint(|ep| unsafe { ep.SetMute(muted, std::ptr::null()) })
            .map_err(|e| format!("Silencio: {e}"))
    })
    .await
}

fn device_id(device: &IMMDevice) -> Result<String, String> {
    unsafe { device.GetId() }
        .map_err(|e| format!("Identificador del dispositivo: {e}"))
        .map(|p| {
            let s = if p.0.is_null() {
                String::new()
            } else {
                unsafe { PCWSTR(p.0).to_string().unwrap_or_default() }
            };
            unsafe { CoTaskMemFree(Some(p.0 as *const core::ffi::c_void)) };
            s
        })
}

fn device_name(device: &IMMDevice) -> Option<String> {
    unsafe {
        let store = device.OpenPropertyStore(STGM_READ).ok()?;
        let mut pv: PROPVARIANT = store.GetValue(&PKEY_Device_FriendlyName).ok()?;
        let name = propvariant_wstring(&pv);
        let _ = PropVariantClear(&mut pv as *mut PROPVARIANT);
        name
    }
}

fn default_output_id() -> Option<String> {
    let _com = ComGuard::new();
    let enumerator: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()? };
    let device: IMMDevice = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eConsole).ok()? };
    device_id(&device).ok()
}

fn output_devices() -> Result<Vec<AudioDevice>, String> {
    let _com = ComGuard::new();
    let enumerator: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
            .map_err(|e| format!("Audio: {e}"))?;
    let collection: IMMDeviceCollection = unsafe { enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) }
        .map_err(|e| format!("Dispositivos de audio: {e}"))?;
    let count = unsafe { collection.GetCount() }
        .map_err(|e| format!("Dispositivos de audio: {e}"))?;
    let default_id = default_output_id();

    let mut devices = Vec::new();
    for i in 0..count {
        let device: IMMDevice = match unsafe { collection.Item(i) } {
            Ok(d) => d,
            Err(_) => continue,
        };
        let id = match device_id(&device) {
            Ok(id) => id,
            Err(_) => continue,
        };
        let name = device_name(&device).unwrap_or_else(|| format!("Altavoces ({i})"));
        let default = default_id.as_deref() == Some(id.as_str());
        devices.push(AudioDevice { id, name, default });
    }
    Ok(devices)
}

#[tauri::command]
pub async fn audio_output_devices() -> Result<Vec<AudioDevice>, String> {
    blocking(output_devices).await
}

const CLSID_POLICY_CONFIG: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);
const IID_POLICY_CONFIG: GUID = GUID::from_u128(0xf8679f50_850a_41cf_9c72_430f290290c8);
const CLSID_POLICY_CONFIG_VISTA: GUID = GUID::from_u128(0x294935ce_f637_4e7c_a41b_ab255460b862);
const IID_POLICY_CONFIG_VISTA: GUID = GUID::from_u128(0x568b9108_44bf_40b4_9006_86afe5b5a620);

type AnyAbiFn = unsafe extern "system" fn() -> isize;

#[repr(C)]
struct PolicyConfigVtbl {
    query_interface: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const GUID,
        *mut *mut core::ffi::c_void,
    ) -> i32,
    add_ref: unsafe extern "system" fn(*mut core::ffi::c_void) -> u32,
    release: unsafe extern "system" fn(*mut core::ffi::c_void) -> u32,
    _passthrough: [AnyAbiFn; 10],
    set_default_endpoint:
        unsafe extern "system" fn(*mut core::ffi::c_void, PCWSTR, i32) -> i32,
    _set_endpoint_visibility: AnyAbiFn,
}

fn with_policy_config(
    f: impl FnOnce(*mut core::ffi::c_void, *const PolicyConfigVtbl) -> windows::core::Result<()>,
) -> Result<(), String> {
    let _com = ComGuard::new();
    for (clsid, iid) in [
        (CLSID_POLICY_CONFIG, IID_POLICY_CONFIG),
        (CLSID_POLICY_CONFIG_VISTA, IID_POLICY_CONFIG_VISTA),
    ] {
        let unk: IUnknown = match unsafe { CoCreateInstance(&clsid, None, CLSCTX_ALL) } {
            Ok(u) => u,
            Err(_) => continue,
        };
        let base = unk.as_raw();
        let base_vt = unsafe { *(base as *const *const PolicyConfigVtbl) };
        let mut raw: *mut core::ffi::c_void = std::ptr::null_mut();
        let hr = unsafe { ((*base_vt).query_interface)(base, &iid, &mut raw) };
        if hr < 0 || raw.is_null() {
            continue;
        }
        let vt = unsafe { *(raw as *const *const PolicyConfigVtbl) };
        let result = f(raw, vt);
        unsafe { ((*vt).release)(raw) };
        return result.map_err(|e| format!("Dispositivo de salida: {e}"));
    }
    Err("No se pudo cambiar el dispositivo de salida (interfaz no disponible)".to_string())
}

fn set_default_endpoint(id: &str) -> Result<(), String> {
    let wide: Vec<u16> = id.encode_utf16().chain(std::iter::once(0)).collect();
    let mut first_error: Option<String> = None;
    let mut any_ok = false;
    for role in [eConsole, eMultimedia, eCommunications] {
        let result = with_policy_config(|raw, vt| unsafe {
            let hr = ((*vt).set_default_endpoint)(raw, PCWSTR(wide.as_ptr()), role.0);
            if hr < 0 {
                Err(windows::core::Error::from(windows::core::HRESULT(hr)))
            } else {
                Ok(())
            }
        });
        match result {
            Ok(()) => any_ok = true,
            Err(e) => {
                first_error.get_or_insert(e);
            }
        }
    }
    if any_ok {
        Ok(())
    } else {
        Err(first_error.unwrap_or_else(|| "No se pudo cambiar el dispositivo de salida".into()))
    }
}

#[tauri::command]
pub async fn audio_set_output_device(id: String) -> Result<(), String> {
    blocking(move || set_default_endpoint(&id)).await
}
