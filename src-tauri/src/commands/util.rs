use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use windows::Security::Cryptography::CryptographicBuffer;
use windows::Storage::Streams::{IRandomAccessStreamReference, InputStreamOptions};
use windows::Win32::System::Com::StructuredStorage::{PROPVARIANT, PROPVARIANT_0_0};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, CoCreateInstance, CoInitializeEx, CoUninitialize,
    COINIT_APARTMENTTHREADED, COINIT_MULTITHREADED,
};
use windows::Win32::System::Variant::{
    VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VariantClear, VT_BSTR, VT_UI4,
};
use windows::Win32::System::Wmi::{ISWbemLocator, ISWbemObject, ISWbemServices};
use windows::core::{Array, BSTR, GUID, HRESULT};

pub struct ComGuard {
    owned: bool,
}

impl ComGuard {
    /// `S_OK` (0) indica que **este** guard es el propietario del `CoInitializeEx`.
    /// `S_FALSE` (1) significa que el hilo ya estaba inicializado (p. ej. por un
    /// guard externo o por el propio hilo): en ese caso desinicializar destruiría
    /// el apartamento con objetos COM todavía vivos.
    pub fn new() -> Self {
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        Self { owned: hr == HRESULT(0) }
    }

    /// Inicializa el hilo como STA: clases como la del portapapeles lo exigen.
    pub fn new_sta() -> Self {
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        Self { owned: hr == HRESULT(0) }
    }
}

/// Ejecuta `f` en un hilo propio con COM en modo STA y devuelve su resultado.
pub fn with_sta<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    std::thread::spawn(move || {
        let _sta = ComGuard::new_sta();
        f()
    })
    .join()
    .map_err(|_| "La operación se interrumpió".to_string())
}

/// Versión asíncrona de [`with_sta`]: ejecuta `f` en un hilo STA sin bloquear
/// el hilo del runtime de la aplicación.
pub async fn sta_async<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let resultado =
        tauri::async_runtime::spawn_blocking(move || with_sta(f))
            .await
            .map_err(|e| format!("Error: {e}"))?;
    resultado.and_then(|inner| inner)
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.owned {
            unsafe { CoUninitialize() };
        }
    }
}

unsafe fn waker_clone(ptr: *const ()) -> RawWaker {
    unsafe {
        Arc::increment_strong_count(ptr as *const std::thread::Thread);
        RawWaker::new(ptr, &WAKE_VTABLE)
    }
}

unsafe fn waker_wake(ptr: *const ()) {
    unsafe {
        (*(ptr as *const std::thread::Thread)).unpark();
        drop(Arc::from_raw(ptr as *const std::thread::Thread));
    }
}

unsafe fn waker_wake_by_ref(ptr: *const ()) {
    unsafe {
        (*(ptr as *const std::thread::Thread)).unpark();
    }
}

unsafe fn waker_drop(ptr: *const ()) {
    unsafe {
        drop(Arc::from_raw(ptr as *const std::thread::Thread));
    }
}

static WAKE_VTABLE: RawWakerVTable =
    RawWakerVTable::new(waker_clone, waker_wake, waker_wake_by_ref, waker_drop);

/// Ejecuta trabajo bloqueante (WMI, `powercfg`, recorridos de disco…) fuera de
/// los *workers* del runtime asíncrono de Tauri, que de otro modo quedarían
/// colgados para toda la aplicación.
pub async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| format!("La operación se interrumpió: {e}"))?
}

/// Bloquea el hilo actual hasta completar un futuro WinRT/COM (no depende del runtime de Tauri).
pub fn block_on<F: Future>(fut: F) -> F::Output {
    let mut fut = std::pin::pin!(fut);
    let me = Arc::new(std::thread::current());
    let ptr = Arc::into_raw(me);
    let waker = unsafe { Waker::from_raw(RawWaker::new(ptr as *const (), &WAKE_VTABLE)) };
    let mut cx = Context::from_waker(&waker);
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => std::thread::park(),
        }
    }
}

fn variant_parts(v: &VARIANT) -> *const VARIANT_0_0 {
    core::ptr::addr_of!((*v).Anonymous.Anonymous) as *const VARIANT_0_0
}

pub fn variant_u32(value: u32) -> VARIANT {
    VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: core::mem::ManuallyDrop::new(VARIANT_0_0 {
                vt: VT_UI4,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: VARIANT_0_0_0 { ulVal: value },
            }),
        },
    }
}

pub fn variant_bstr(value: &str) -> VARIANT {
    VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: core::mem::ManuallyDrop::new(VARIANT_0_0 {
                vt: VT_BSTR,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: VARIANT_0_0_0 {
                    bstrVal: core::mem::ManuallyDrop::new(BSTR::from(value)),
                },
            }),
        },
    }
}

/// `VARIANT` con liberación automática. `windows-rs` **no** implementa `Drop`
/// para `VARIANT`, así que sin esto el `BSTR` que contiene se fuga.
pub struct Variant(VARIANT);

impl Variant {
    pub fn bstr(value: &str) -> Self {
        Self(variant_bstr(value))
    }

    pub fn u32(value: u32) -> Self {
        Self(variant_u32(value))
    }

    pub fn get(&self) -> &VARIANT {
        &self.0
    }
}

impl Drop for Variant {
    fn drop(&mut self) {
        unsafe {
            let _ = VariantClear(&mut self.0);
        }
    }
}

pub unsafe fn variant_as_u32(v: &VARIANT) -> Option<u32> {
    unsafe {
        let inner = variant_parts(v);
        let value = match (*inner).vt.0 {
            17u16 => (*inner).Anonymous.bVal as u32,
            2u16 => (*inner).Anonymous.iVal as u32,
            18u16 => (*inner).Anonymous.uiVal as u32,
            3u16 => (*inner).Anonymous.lVal as u32,
            19u16 => (*inner).Anonymous.ulVal,
            _ => return None,
        };
        Some(value)
    }
}

pub unsafe fn propvariant_wstring(pv: &PROPVARIANT) -> Option<String> {
    unsafe {
        let inner =
            core::ptr::addr_of!((*pv).Anonymous.Anonymous) as *const PROPVARIANT_0_0;
        match (*inner).vt.0 {
            31u16 => {
                let ptr = (*inner).Anonymous.pwszVal.0;
                if ptr.is_null() {
                    None
                } else {
                    windows::core::PCWSTR(ptr).to_string().ok()
                }
            }
            8u16 => {
                let bstr = &(*inner).Anonymous.bstrVal;
                if bstr.is_empty() {
                    None
                } else {
                    Some(bstr.to_string())
                }
            }
            _ => None,
        }
    }
}

/// Toma **propiedad** de `v`: lee el `BSTR` y libera el `VARIANT` (que no tiene
/// `Drop` en `windows-rs`). Llamar siempre con el valor devuelto por `Value()`.
pub unsafe fn variant_as_string(mut v: VARIANT) -> Option<String> {
    let out = unsafe {
        let inner = variant_parts(&v);
        match (*inner).vt.0 {
            8u16 => Some((*inner).Anonymous.bstrVal.to_string()),
            _ => None,
        }
    };
    unsafe {
        let _ = VariantClear(&mut v);
    }
    out
}

pub fn wmi_services(service: &str) -> Result<ISWbemServices, String> {
    // SWbemLocator (wbemdisp): clase que expone la interfaz ISWbemLocator.
    const CLSID_SWBEM_LOCATOR: GUID = GUID::from_u128(0x76a64158_cb41_11d1_8b02_00600806d9b6);
    let locator: ISWbemLocator =
        unsafe { CoCreateInstance(&CLSID_SWBEM_LOCATOR, None, CLSCTX_INPROC_SERVER) }
            .map_err(|e| format!("WMI: {e}"))?;
    unsafe {
        locator.ConnectServer(
            &BSTR::from(""),
            &BSTR::from(service),
            &BSTR::from(""),
            &BSTR::from(""),
            &BSTR::from(""),
            &BSTR::from(""),
            0,
            None,
        )
    }
    .map_err(|e| format!("WMI ({service}): {e}"))
}

pub fn wmi_query(svc: &ISWbemServices, query: &str) -> Result<Option<ISWbemObject>, String> {
    // La interfaz de automatización (wbemdisp) rechaza las banderas
    // FORWARD_ONLY | RETURN_IMMEDIATELY (0x11) con "Parámetro no válido".
    let set = unsafe {
        svc.ExecQuery(
            &BSTR::from(query),
            &BSTR::from("WQL"),
            0,
            None,
        )
    }
    .map_err(|e| format!("WMI: {e}"))?;
    Ok(unsafe { set.ItemIndex(0) }.ok())
}

/// Convierte marcas de tiempo WinRT (100 ns desde 1601) a milisegundos Unix.
pub fn ticks_to_ms(ticks: i64) -> Option<u64> {
    const EPOCH_1601: i64 = 116_444_736_000_000_000;
    ticks
        .checked_sub(EPOCH_1601)
        .filter(|v| *v >= 0)
        .map(|v| (v / 10_000) as u64)
}

/// Lee un flujo WinRT y lo devuelve como data URL base64 (solo imágenes).
pub fn stream_reference_data_url(
    stream_ref: &IRandomAccessStreamReference,
    max_bytes: u64,
) -> Option<String> {
    let open = stream_ref.OpenReadAsync().ok()?;
    let stream = block_on(async move { open.await }).ok()?;
    let size = stream.Size().ok()?;
    if size == 0 || size > max_bytes {
        return None;
    }
    let mime = stream
        .ContentType()
        .ok()
        .map(|h| h.to_string())
        .filter(|s| s.starts_with("image/"))
        .unwrap_or_else(|| "image/png".to_string());
    let zeros = vec![0u8; size as usize];
    let in_buf = CryptographicBuffer::CreateFromByteArray(&zeros).ok()?;
    let read = stream
        .ReadAsync(&in_buf, size as u32, InputStreamOptions(0))
        .ok()?;
    let out_buf = block_on(async move { read.await }).ok()?;
    let mut arr = Array::with_len(size as usize);
    CryptographicBuffer::CopyToByteArray(&out_buf, &mut arr).ok()?;
    Some(format!("data:{mime};base64,{}", STANDARD.encode(&*arr)))
}
