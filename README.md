# Koyun

**Panel de control del sistema para Windows** â€” una app de escritorio ligera (Tauri 2 + React 19 + Tailwind v4) que controla volumen, energÃ­a, red,Bluetooth, portapapeles y fondos de pantalla, con una **isla dinÃ¡mica** que aparece al pasar el ratÃ³n bajo la cÃ¡mara.


---

## Ãndice

- [CaracterÃ­sticas](#caracterÃ­sticas)
- [InstalaciÃ³n](#instalaciÃ³n)
- [Uso](#uso)
- [Desarrollo](#desarrollo)
- [CÃ³mo funciona por dentro](#cÃ³mo-funciona-por-dentro)
- [Estructura del proyecto](#estructura-del-proyecto)
- [Comandos Tauri](#comandos-tauri)
- [SoluciÃ³n de problemas](#soluciÃ³n-de-problemas)
- [Licencia](#licencia)

---

## CaracterÃ­sticas

### ðŸ  Fondo de pantalla
- **ImÃ¡genes y vÃ­deos** locales desde `%APPDATA%\koyun\fondos`.
- **Fondo en movimiento real**: el vÃ­deo se reproduce en una ventana hija del `WorkerW` del escritorio, es decir, **detrÃ¡s de los iconos**, y no intercepta el ratÃ³n.
- **Fondo distinto por monitor** usando la API `IDesktopWallpaper` de Windows. El selector detecta **todos** los monitores conectados (principal, secundarios, modelo y resoluciÃ³n) y permite fijar una imagen distinta en cada uno.
- **Miniaturas reales**: las imÃ¡genes se decodifican en Rust (crate `image`) y se sirven como `data:` URL, con cachÃ© en el frontend.
- **RotaciÃ³n automÃ¡tica** de fondos con intervalo configurable.
- Etiqueta en cada miniatura indicando **en quÃ© pantalla(s)** estÃ¡ puesto ese fondo.

### ðŸŒ Ajustes rÃ¡pidos
Un cajÃ³n lateral dentro de la propia app (no abre los ajustes de Windows) con acciones **reales**:

| SecciÃ³n | QuÃ© hace |
|---|---|
| **Wi-Fi** | Lista las redes disponibles, muestra la seÃ±al, si tienen **perfil guardado** y si estÃ¡n conectadas. Un clic conecta (`netsh wlan connect name=`) y hay botÃ³n para desconectar. |
| **Bluetooth** | Lista dispositivos emparejados y emparejables, con botones **Conectar** y **Quitar**. |
| **EnergÃ­a** | Muestra la **baterÃ­a** (nivel y estado de carga) y permite cambiar el **plan de energÃ­a** (Equilibrado / Alto rendimiento / etc.). |

### ðŸ”Š Controles
- Volumen (con barra arrastrable que no se interrumpe al hacer polling).
- Silenciar, elegir dispositivo de salida de audio.
- Radios: Wi-Fi, Bluetooth, modo aviÃ³n.
- Brillo, notas adhesivas, ahorro de energÃ­a, modo Â«No molestarÂ».
- **CorrecciÃ³n de un bug real**: aplicar un radio ya activo devolvÃ­a `0x8007139F` (`ERROR_INVALID_STATE`); ahora la operaciÃ³n es idempotente.

### âš¡ EnergÃ­a y sistema
- Bloquear, suspender, reiniciar y apagar.
- Plans de energÃ­a, CPU/GPU/RAM en uso, temperatura y Media Session.

### ðŸ“‹ Portapapeles
- Historial, restauraciÃ³n y borrado individual o completo.

### ðŸ”˜ Isla dinÃ¡mica
- Una pastilla oscura tipo *notch* bajo la cÃ¡mara que se expande y muestra el **logo de Koyun** al pasar el ratÃ³n.
- Al pulsarla abre/cierra el panel principal.

---

## InstalaciÃ³n

1. Descarga `koyun_0.1.0_x64-setup.exe` de la secciÃ³n **Releases**.
2. EjecÃºtalo (instalador **NSIS**; se instala en `%LOCALAPPDATA%\koyun`).
3. Se aÃ±ade un **icono en la bandeja del sistema** desde el que puedes mostrar, ocultar o salir.

> **Requisito:** Windows 10 o superior. En Windows 11 se aplica ademÃ¡s el redondeo nativo de las esquinas de la ventana.

---

## Uso

| AcciÃ³n | CÃ³mo |
|---|---|
| Mostrar / ocultar el panel | `Ctrl+Alt+K`, clic en la isla, o clic en el icono de la bandeja |
| Cambiar de pestaÃ±a | `Ctrl+1` â€¦ `Ctrl+5` |
| Cerrar el panel | `Esc` (lo oculta, no termina la app) |
| Salir de la app | MenÃº del icono de bandeja â†’ **Salir** |

---

## Desarrollo

### Requisitos
- [Node.js](https://nodejs.org/) 18+
- [Rust](https://www.rust-lang.org/tools/install) (toolchain estable)
- [Tauri 2](https://tauri.app/start/prerequisites/) y las dependencias de WebView2 (ya incluidas en Windows 10/11)

### Puesta en marcha

```bash
npm install
npm run tauri dev
```

### Compilar

```bash
# Comprobar tipos + bundle del frontend
npm run build

# Instalador NSIS de release (en src-tauri/target/release/bundle/nsis)
npm run tauri build
```

### Instalador silentoso (sin interfaz)

```powershell
Start-Process "src-tauri\target\release\bundle\nsis\koyun_0.1.0_x64-setup.exe" -ArgumentList "/S"
Start-Process "$env:LOCALAPPDATA\koyun\koyun.exe"
```

---

## CÃ³mo funciona por dentro

### Las tres ventanas

| Ventana | HTML | PropÃ³sito |
|---|---|---|
| `main` | `index.html` | El panel principal (780Ã—520, sin decoraciones, siempre encima). |
| `island` | `island.html` | La isla dinÃ¡mica (56Ã—56, transparente, sin foco). Se muestra solo cuando el cursor entra en la zona superior del monitor. |
| `wallpaper` | `wallpaper.html` | Reproductor del vÃ­deo de fondo. Se reparenta al `WorkerW` del escritorio. |

### Fondo en movimiento

`src-tauri/src/commands/videowallpaper.rs` usa el truco clÃ¡sico de Windows:

1. EnvÃ­a `0x052C` (`WM_USER`) al `Progman` para que cree el `WorkerW` de los iconos.
2. Localiza `SHELLDLL_DefView` y su `WorkerW` padre.
3. `SetParent` sobre la ventana `wallpaper` y `GetClientRect` + `SetWindowPos` para ajustarla al Ã¡rea del escritorio.
4. `set_ignore_cursor_events(true)` para que el vÃ­deo no robe el ratÃ³n.

El estado se persiste en `%APPDATA%\koyun\fondo-animado.json` y se **reanuda automÃ¡ticamente** al arrancar la app.

### Fondo por monitor

`src-tauri/src/commands/wallpaper.rs` instancia la interfaz COM `IDesktopWallpaper` para:

- `GetMonitorDevicePathCount()` â†’ nÃºmero real de monitores (soporta N).
- `GetMonitorDevicePathAt(i)` â†’ identificador del monitor; de ahÃ­ se extrae el **modelo** (`AOC2243`, `CMN15E7`â€¦).
- `GetMonitorRECT()` â†’ resoluciÃ³n.
- `SetWallpaper(id, path)` â†’ aplicar a una pantalla concreta.

### Redondeo de las esquinas

La ventana principal usa **redondeo nativo de Windows** (`DwmSetWindowAttribute` con `DWMWA_WINDOW_CORNER_PREFERENCE = DWMWCP_ROUND`), no transparencia. Esto evita el marco blanco que aparecÃ­a al activar `transparent: true` en el WebView2.

### Communication

Todo el acceso al sistema ocurre en **Rust** (crate `windows` 0.62). El frontend solo llama a comandos `invoke("nombre", { args })` registrados en `src-tauri/src/lib.rs`.

Los **errores** se registran en `%LOCALAPPDATA%\koyun\koyun.log` (con rotaciÃ³n por tamaÃ±o) para poder diagnosticar sin depurador.

---

## Estructura del proyecto

```
koyun/
â”œâ”€â”€ index.html              # Ventana principal
â”œâ”€â”€ island.html             # Isla dinÃ¡mica
â”œâ”€â”€ wallpaper.html          # Reproductor de fondo
â”œâ”€â”€ vite.config.ts          # Entradas: index / island / wallpaper
â”œâ”€â”€ src/
â”‚   â”œâ”€â”€ main.tsx            # Bootstrap del panel
â”‚   â”œâ”€â”€ App.tsx             # Layout: TopBar + pestaÃ±as
â”‚   â”œâ”€â”€ styles.css          # Tailwind v4 + estilos globales
â”‚   â”œâ”€â”€ components/
â”‚   â”‚   â”œâ”€â”€ TopBar.tsx
â”‚   â”‚   â”œâ”€â”€ QuickSettings.tsx      # CajÃ³n de ajustes rÃ¡pidos
â”‚   â”‚   â”œâ”€â”€ icons.tsx
â”‚   â”‚   â””â”€â”€ tabs/                    # Controls / Power / Stats / Clipboard / Wallpaper
â”‚   â”œâ”€â”€ island/main.tsx     # LÃ³gica de la isla
â”‚   â”œâ”€â”€ island/logo_isla.png
â”‚   â”œâ”€â”€ wallpaper/main.tsx  # Reproductor de vÃ­deo
â”‚   â”œâ”€â”€ store/              # Zustand (ui, settings)
â”‚   â””â”€â”€ lib/                # Utilidades (polling, formato, errores, log)
â””â”€â”€ src-tauri/
    â”œâ”€â”€ tauri.conf.json     # Ventanas, permisos, bundle NSIS
    â”œâ”€â”€ capabilities/default.json
    â””â”€â”€ src/
        â”œâ”€â”€ lib.rs          # Builder, setup, atajo global, bandeja, comandos
        â””â”€â”€ commands/
            â”œâ”€â”€ audio.rs        # Volumen y salidas de audio
            â”œâ”€â”€ clipboard.rs    # Historial del portapapeles
            â”œâ”€â”€ controls.rs     # Radios, tiles, brillo, powercfg
            â”œâ”€â”€ island.rs       # Isla dinÃ¡mica y tracker del ratÃ³n
            â”œâ”€â”€ log.rs
            â”œâ”€â”€ media.rs        # Media Session
            â”œâ”€â”€ power.rs        # Bloquear / suspender / reiniciar / apagar
            â”œâ”€â”€ quick.rs        # Ajustes rÃ¡pidos (Wi-Fi, BT, energÃ­a)
            â”œâ”€â”€ stats.rs        # CPU / GPU / RAM / temperatura
            â”œâ”€â”€ util.rs
            â”œâ”€â”€ videowallpaper.rs  # Fondo en movimiento
            â””â”€â”€ wallpaper.rs       # Fondos, miniaturas y fondos por monitor
```

---

## Comandos Tauri

<details>
<summary>Ver todos</summary>

**Foncos**

- `wallpaper_list` â€” lista los fondos disponibles
- `wallpaper_thumbnail` â€” genera una miniatura `data:` URL
- `wallpaper_monitors` â€” devuelve los monitores detectados
- `wallpaper_set_monitor` â€” aplica un fondo a un monitor concreto
- `wallpaper_current` â€” devuelve el fondo activo
- `wallpaper_set` â€” aplica un fondo a todas las pantallas
- `wallpaper_import` / `wallpaper_delete` â€” importa o borra fondos
- `wallpaper_video_start` / `wallpaper_video_stop` / `wallpaper_video_state` â€” fondo en movimiento

**Ajustes rÃ¡pidos**

- `quick_network_detail`, `quick_wifi_networks`, `quick_wifi_connect`, `quick_wifi_disconnect`
- `quick_bluetooth_devices`, `quick_bluetooth_pair`, `quick_bluetooth_unpair`
- `quick_battery`, `quick_power_plans`, `quick_power_plan_set`

**Controles y sistema**

- `controls_volume_get/set/set_mute`, `controls_network`, `controls_tiles_state`
- `controls_wifi_set`, `controls_bluetooth_set`, `controls_airplane_set`, `controls_sticky_set`, `controls_saver_set`, `controls_dnd_set`
- `controls_brightness_get/set`
- `audio_output_devices`, `audio_set_output_device`
- `media_info`, `media_action`
- `stats_snapshot`
- `clipboard_history`, `clipboard_restore`, `clipboard_delete`, `clipboard_clear`
- `power_lock`, `power_suspend`, `power_restart`, `power_shutdown`
- `island_toggle_main`, `log_message`

</details>

---

## SoluciÃ³n de problemas

| SÃ­ntoma | Causa y soluciÃ³n |
|---|---|
| Una red aparece sin poder conectarse | Windows exige un **perfil guardado**. Koyun lo indica con la etiqueta Â«Sin perfilÂ»: conÃ©ctala una vez desde Windows. |
| El vÃ­deo de fondo no aparece | Ocurre al arrancar en Windows 11 multiâ€‘monitor. Reinicia la app; el estado se reanuda solo. |
| El atajo `Ctrl+Alt+K` no responde | Otra aplicaciÃ³n puede tener registrado el atajo. Koyun avisa con un diÃ¡logo y sigue usando el icono de la bandeja. |
| El fondo de un monitor no cambia | AsegÃºrate de que la imagen estÃ© en una ruta accesible y de que el monitor siga conectado. |
| Los avisos de Â«perfilÂ» / emparejamiento tardan | Los dispositivos Bluetooth necesitan que la radio estÃ© encendida (se puede desde el propio panel). |

Los errores tÃ©cnicos quedan registrados en `%LOCALAPPDATA%\koyun\koyun.log`.

---

## Licencia

MIT

---

### Mezclador por aplicaciÃ³n
- Controla el volumen y silenciado por sesiÃ³n de audio (aplicaciones) con actualizaciÃ³n en tiempo real.
- Control de micrÃ³fono con silenciado directo.

### Historial de notificaciones
- Lista las notificaciones activas de Windows mediante `UserNotificationListener`.
- Acceso rÃ¡pido a ajustes de notificaciones del sistema.

### Ajustes unificados
- Apariencia: modo claro/oscuro/sistema, acento y paletas personalizables.
- Atajos globales configurables por acciÃ³n (captura directa, deduplicado, validaciÃ³n).
- Opciones de isla dinÃ¡mica y notificaciones.

### Atajos globales configurables
- Se migraron los atajos fijos a configuraciÃ³n unificada en `%APPDATA%\koyun\config.json`.
- Registro dinÃ¡mico desde Ajustes sin recompilar.
