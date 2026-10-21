use serde::Serialize;
use windows::Devices::Bluetooth::BluetoothDevice;
use windows::Devices::Enumeration::DeviceInformation;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::NetworkManagement::WiFi::{
    WLAN_AVAILABLE_NETWORK_LIST, WlanCloseHandle, WlanEnumInterfaces, WlanFreeMemory,
    WlanGetAvailableNetworkList, WlanOpenHandle,
};
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::core::HSTRING;

use crate::commands::util::{ComGuard, blocking};

#[derive(Serialize)]
pub struct NetworkDetail {
    pub name: Option<String>,
    pub ssid: Option<String>,
    pub signal: Option<u32>,
    pub internet: bool,
    pub ipv4: Option<String>,
    pub gateway: Option<String>,
    pub dns: Vec<String>,
    pub adapter: Option<String>,
    pub security: Option<String>,
}

#[derive(Serialize)]
pub struct WifiNetwork {
    pub ssid: String,
    pub signal: u32,
    pub secure: bool,
    pub connected: bool,
    pub profile: bool,
}

#[derive(Serialize)]
pub struct BluetoothDeviceInfo {
    pub id: String,
    pub name: String,
    pub connected: bool,
    pub kind: String,
}

#[derive(Serialize)]
pub struct BatteryInfo {
    pub present: bool,
    pub charging: bool,
    pub percent: Option<u8>,
    pub minutes_left: Option<u32>,
}

#[derive(Serialize)]
pub struct PowerPlan {
    pub guid: String,
    pub name: String,
    pub active: bool,
}

// ---------------------------------------------------------------------------
// Red
// ---------------------------------------------------------------------------

fn powershell_json(script: &str) -> Result<String, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("No se pudo consultar la red: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn network_detail_inner() -> Result<NetworkDetail, String> {
    // Un único PowerShell evita depender del adaptador concreto y funciona tanto
    // en Wi-Fi como en Ethernet.
    let script = r#"
$ErrorActionPreference='SilentlyContinue'
$cfg = Get-NetIPConfiguration | Where-Object { $_.NetProfile.IPv4Connectivity -ne 'Disconnected' } | Select-Object -First 1
if (-not $cfg) { $cfg = Get-NetIPConfiguration | Where-Object { $_.IPv4DefaultGateway } | Select-Object -First 1 }
$wifi = netsh wlan show interfaces | Out-String
$ssid = ([regex]::Match($wifi,'(?m)^\s*SSID\s*:\s*(.+)$')).Groups[1].Value.Trim()
$sig  = ([regex]::Match($wifi,'(?m)^\s*Se[^\:]*:\s*(\d+)%')).Groups[1].Value.Trim()
$auth = ([regex]::Match($wifi,'(?m)^\s*Authentication\s*:\s*(.+)$')).Groups[1].Value.Trim()
[pscustomobject]@{
  name = $cfg.InterfaceAlias
  ssid = $ssid
  signal = if ($sig) { [int]$sig } else { $null }
  internet = ($cfg.NetProfile.IPv4Connectivity -eq 'Internet')
  ipv4 = ($cfg.IPv4Address.IPAddress | Select-Object -First 1)
  gateway = ($cfg.IPv4DefaultGateway.NextHop | Select-Object -First 1)
  dns = @($cfg.DNSServer | Where-Object { $_.AddressFamily -eq 2 } | ForEach-Object { $_.ServerAddresses } | Select-Object -First 3)
  security = $auth
} | ConvertTo-Json -Compress
"#;
    let raw = powershell_json(script)?;
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(NetworkDetail {
            name: None,
            ssid: None,
            signal: None,
            internet: false,
            ipv4: None,
            gateway: None,
            dns: Vec::new(),
            adapter: None,
            security: None,
        });
    }
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|e| format!("Red: respuesta no válida ({e})"))?;
    let text = |v: &serde_json::Value, key: &str| -> Option<String> {
        v.get(key)
            .and_then(|x| x.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    Ok(NetworkDetail {
        name: text(&value, "name"),
        ssid: text(&value, "ssid"),
        signal: value.get("signal").and_then(|v| v.as_u64()).map(|v| v as u32),
        internet: value
            .get("internet")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        ipv4: text(&value, "ipv4"),
        gateway: text(&value, "gateway"),
        dns: value
            .get("dns")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        adapter: text(&value, "name"),
        security: text(&value, "security"),
    })
}

#[tauri::command]
pub async fn quick_network_detail() -> Result<NetworkDetail, String> {
    blocking(network_detail_inner).await
}

fn wifi_networks_inner() -> Result<Vec<WifiNetwork>, String> {
    let profiles = wifi_profiles();
    let mut negotiated = 0u32;
    let mut handle = HANDLE(std::ptr::null_mut());
    if unsafe { WlanOpenHandle(2, None, &mut negotiated, &mut handle) } != 0 {
        return Err("El servicio Wi-Fi no responde".to_string());
    }

    let mut out: Vec<WifiNetwork> = Vec::new();
    let mut iface_list: *mut windows::Win32::NetworkManagement::WiFi::WLAN_INTERFACE_INFO_LIST =
        std::ptr::null_mut();

    if unsafe { WlanEnumInterfaces(handle, None, &mut iface_list) } == 0 && !iface_list.is_null() {
        let iface_count = unsafe { (*iface_list).dwNumberOfItems };
        let base = unsafe { std::ptr::addr_of!((*iface_list).InterfaceInfo) }
            as *const windows::Win32::NetworkManagement::WiFi::WLAN_INTERFACE_INFO;
        for i in 0..iface_count {
            let info = unsafe { &*base.add(i as usize) };
            let mut net_list: *mut WLAN_AVAILABLE_NETWORK_LIST = std::ptr::null_mut();
            if unsafe { WlanGetAvailableNetworkList(handle, &info.InterfaceGuid, 0, None, &mut net_list) }
                != 0
                || net_list.is_null()
            {
                continue;
            }
            let count = unsafe { (*net_list).dwNumberOfItems };
            let net_base = unsafe { std::ptr::addr_of!((*net_list).Network) }
                as *const windows::Win32::NetworkManagement::WiFi::WLAN_AVAILABLE_NETWORK;
            for j in 0..count {
                let net = unsafe { &*net_base.add(j as usize) };
                let len = (net.dot11Ssid.uSSIDLength as usize).min(net.dot11Ssid.ucSSID.len());
                if len == 0 {
                    continue;
                }
                let ssid = String::from_utf8_lossy(&net.dot11Ssid.ucSSID[..len]).into_owned();
                // `dwFlags` bit 0 = conectada; la seguridad se deduce del tipo de
                // autenticación (1 = abierta).
                let connected = net.dwFlags & 0x1 != 0;
                let secure = net.bSecurityEnabled.as_bool();
                if let Some(existing) = out.iter_mut().find(|n| n.ssid == ssid) {
                    existing.signal = existing.signal.max(net.wlanSignalQuality);
                    existing.connected |= connected;
                } else {
                    let has_profile = profiles.iter().any(|p| p.eq_ignore_ascii_case(&ssid));
                    out.push(WifiNetwork {
                        ssid,
                        signal: net.wlanSignalQuality,
                        secure,
                        connected,
                        profile: has_profile,
                    });
                }
            }
            unsafe { WlanFreeMemory(net_list as _) };
        }
        unsafe { WlanFreeMemory(iface_list as _) };
    }

    unsafe { WlanCloseHandle(handle, None) };
    out.sort_by(|a, b| b.connected.cmp(&a.connected).then(b.signal.cmp(&a.signal)));
    Ok(out)
}

#[tauri::command]
pub async fn quick_wifi_networks() -> Result<Vec<WifiNetwork>, String> {
    blocking(wifi_networks_inner).await
}

/// Nombres de los perfiles Wi-Fi guardados (`netsh wlan show profiles`).
fn wifi_profiles() -> Vec<String> {
    let Ok(out) = netsh(&["wlan", "show", "profiles"]) else {
        return Vec::new();
    };
    out.lines()
        .filter_map(|line| line.split_once(':').map(|(_, v)| v.trim().to_string()))
        .filter(|name| !name.is_empty() && name != "<Ninguno>")
        .collect()
}

fn netsh(args: &[&str]) -> Result<String, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = std::process::Command::new("netsh")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("No se pudo ejecutar netsh: {e}"))?;
    if !out.status.success() {
        let msg = if out.stderr.is_empty() {
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        } else {
            String::from_utf8_lossy(&out.stderr).trim().to_string()
        };
        return Err(if msg.is_empty() { "netsh falló".to_string() } else { msg });
    }
    // `netsh` usa la página de códigos OEM en la consola: se descodifica como
    // UTF-8 con pérdida, suficiente para nombres de perfil ASCII/Latin-1.
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Conecta a una red Wi-Fi usando un perfil ya guardado. Windows no permite
/// conectar a una red nueva sin crear antes el perfil, así que si no existe se
/// devuelve un error claro en lugar de fallar en silencio.
fn wifi_connect_inner(ssid: &str) -> Result<(), String> {
    let profiles = wifi_profiles();
    let Some(profile) = profiles.iter().find(|p| p.eq_ignore_ascii_case(ssid)) else {
        return Err(format!(
            "No hay un perfil guardado para «{ssid}». Conéctate una vez desde Windows para crearlo."
        ));
    };
    let out = netsh(&["wlan", "connect", &format!("name={profile}")])?;
    let lower = out.to_lowercase();
    if lower.contains("completed") || lower.contains("correctamente") || lower.contains("satisfactor") {
        Ok(())
    } else {
        Err(out.trim().to_string())
    }
}

#[tauri::command]
pub async fn quick_wifi_connect(ssid: String) -> Result<(), String> {
    blocking(move || wifi_connect_inner(&ssid)).await
}

/// Desconecta del Wi-Fi actual.
#[tauri::command]
pub async fn quick_wifi_disconnect() -> Result<(), String> {
    blocking(|| netsh(&["wlan", "disconnect"]).map(|_| ())).await
}

// ---------------------------------------------------------------------------
// Bluetooth
// ---------------------------------------------------------------------------

fn bluetooth_devices_inner() -> Result<Vec<BluetoothDeviceInfo>, String> {
    let _com = ComGuard::new();
    let selector = BluetoothDevice::GetDeviceSelector().map_err(|e| format!("Bluetooth: {e}"))?;
    let op = DeviceInformation::FindAllAsyncAqsFilter(&selector)
        .map_err(|e| format!("Bluetooth: {e}"))?;
    let infos = crate::commands::util::block_on(async move { op.await })
        .map_err(|e| format!("Bluetooth: {e}"))?;

    let count = infos.Size().map_err(|e| format!("Bluetooth: {e}"))?;
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count {
        let info = infos.GetAt(i).map_err(|e| format!("Bluetooth: {e}"))?;
        let id = info.Id().map(|h| h.to_string()).unwrap_or_default();
        let name = info
            .Name()
            .map(|h| h.to_string())
            .unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        let (connected, kind) = match BluetoothDevice::FromIdAsync(&HSTRING::from(&id)) {
            Ok(op) => match crate::commands::util::block_on(async move { op.await }) {
                Ok(device) => {
                    let connected = device
                        .ConnectionStatus()
                        .map(|s| s == windows::Devices::Bluetooth::BluetoothConnectionStatus::Connected)
                        .unwrap_or(false);                    let kind = device
                        .ClassOfDevice()
                        .and_then(|c| c.RawValue())
                        .map(classify_bluetooth)
                        .unwrap_or_else(|_| "Dispositivo".to_string());
                    (connected, kind)
                }
                Err(_) => (false, "Dispositivo".to_string()),
            },
            Err(_) => (false, "Dispositivo".to_string()),
        };
        out.push(BluetoothDeviceInfo {
            id,
            name,
            connected,
            kind,
        });
    }
    out.sort_by(|a, b| b.connected.cmp(&a.connected).then(a.name.cmp(&b.name)));
    Ok(out)
}

/// Clase principal del `ClassOfDevice` (bits 8-12) según la especificación BT.
fn classify_bluetooth(raw: u32) -> String {
    let major = (raw >> 8) & 0x1F;
    match major {
        1 => "Ordenador",
        2 => "Teléfono",
        4 => "Audio",
        5 => "Periférico",
        6 => "Imagen",
        7 => "Wearable",
        _ => "Dispositivo",
    }
    .to_string()
}

#[tauri::command]
pub async fn quick_bluetooth_devices() -> Result<Vec<BluetoothDeviceInfo>, String> {
    blocking(bluetooth_devices_inner).await
}

/// Empareja (y por tanto conecta) un dispositivo Bluetooth ya detectado.
fn bluetooth_pair_inner(id: &str) -> Result<(), String> {
    let _com = ComGuard::new();
    let info = DeviceInformation::CreateFromIdAsync(&HSTRING::from(id))
        .map_err(|e| format!("Bluetooth: {e}"))?;
    let info = crate::commands::util::block_on(async move { info.await })
        .map_err(|e| format!("Bluetooth: {e}"))?;

    let pairing = info.Pairing().map_err(|e| format!("Bluetooth: {e}"))?;
    if pairing.IsPaired().unwrap_or(false) {
        // Ya emparejado: el enlace lo reactiva Windows; aparecerá conectado en
        // el siguiente sondeo, así que no hay nada más que hacer.
        return Ok(());
    }
    let op = pairing.PairAsync().map_err(|e| format!("Bluetooth: {e}"))?;
    let result = crate::commands::util::block_on(async move { op.await })
        .map_err(|e| format!("Bluetooth: {e}"))?;
    let status = result.Status().map_err(|e| format!("Bluetooth: {e}"))?;
    use windows::Devices::Enumeration::DevicePairingResultStatus;
    match status {
        DevicePairingResultStatus::Paired | DevicePairingResultStatus::AlreadyPaired => Ok(()),
        DevicePairingResultStatus::AuthenticationFailure
        | DevicePairingResultStatus::AuthenticationTimeout => {
            Err("La autenticación con el dispositivo falló".to_string())
        }
        DevicePairingResultStatus::ConnectionRejected => {
            Err("El dispositivo rechazó la conexión".to_string())
        }
        other => Err(format!("No se pudo emparejar ({other:?})")),
    }
}

#[tauri::command]
pub async fn quick_bluetooth_pair(id: String) -> Result<(), String> {
    blocking(move || bluetooth_pair_inner(&id)).await
}

/// Quita el emparejamiento (desconecta y olvida el dispositivo).
fn bluetooth_unpair_inner(id: &str) -> Result<(), String> {
    let _com = ComGuard::new();
    let info = DeviceInformation::CreateFromIdAsync(&HSTRING::from(id))
        .map_err(|e| format!("Bluetooth: {e}"))?;
    let info = crate::commands::util::block_on(async move { info.await })
        .map_err(|e| format!("Bluetooth: {e}"))?;
    let pairing = info.Pairing().map_err(|e| format!("Bluetooth: {e}"))?;
    if !pairing.IsPaired().unwrap_or(false) {
        return Ok(());
    }
    let op = pairing.UnpairAsync().map_err(|e| format!("Bluetooth: {e}"))?;
    crate::commands::util::block_on(async move { op.await })
        .map_err(|e| format!("Bluetooth: {e}"))?;
    Ok(())
}

#[tauri::command]
pub async fn quick_bluetooth_unpair(id: String) -> Result<(), String> {
    blocking(move || bluetooth_unpair_inner(&id)).await
}

// ---------------------------------------------------------------------------
// Batería y planes de energía
// ---------------------------------------------------------------------------

fn battery_inner() -> Result<BatteryInfo, String> {
    let mut status = SYSTEM_POWER_STATUS::default();
    unsafe { GetSystemPowerStatus(&mut status) }.map_err(|e| format!("Batería: {e}"))?;

    // BatteryFlag 128 = sin batería; 255 = desconocido.
    let present = status.BatteryFlag != 128 && status.BatteryFlag != 255;
    let charging = status.ACLineStatus == 1;
    let percent = (status.BatteryLifePercent != 255).then_some(status.BatteryLifePercent);
    let minutes_left = (present && !charging && status.BatteryLifeTime != u32::MAX)
        .then_some(status.BatteryLifeTime / 60);

    Ok(BatteryInfo {
        present,
        charging,
        percent,
        minutes_left,
    })
}

#[tauri::command]
pub async fn quick_battery() -> Result<BatteryInfo, String> {
    blocking(battery_inner).await
}

fn power_plans_inner() -> Result<Vec<PowerPlan>, String> {
    use crate::commands::controls::powercfg_list;
    let active = crate::commands::controls::active_plan_guid();
    let mut out: Vec<PowerPlan> = Vec::new();
    for (guid, name) in powercfg_list()? {
        let is_active = active
            .as_deref()
            .is_some_and(|a| a.eq_ignore_ascii_case(&guid));
        out.push(PowerPlan {
            guid,
            name,
            active: is_active,
        });
    }
    Ok(out)
}

#[tauri::command]
pub async fn quick_power_plans() -> Result<Vec<PowerPlan>, String> {
    blocking(power_plans_inner).await
}

#[tauri::command]
pub async fn quick_power_plan_set(guid: String) -> Result<(), String> {
    blocking(move || crate::commands::controls::set_active_plan(&guid)).await
}
