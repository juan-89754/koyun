# Koyun

**Panel de control del sistema para Windows** — una app de escritorio ligera (Tauri 2 + React 19 + Tailwind v4) que controla volumen, energía, red,Bluetooth, portapapeles y fondos de pantalla, con una **isla dinámica** que aparece al pasar el ratón bajo la cámara.

![Koyun](src/island/logo_isla.png)

---

## Índice

- [Características](#características)
- [Instalación](#instalación)
- [Uso](#uso)
- [Desarrollo](#desarrollo)
- [Cómo funciona por dentro](#cómo-funciona-por-dentro)
- [Estructura del proyecto](#estructura-del-proyecto)
- [Comandos Tauri](#comandos-tauri)
- [Solución de problemas](#solución-de-problemas)
- [Licencia](#licencia)

---

## Características

### 🏠 Fondo de pantalla
- **Imágenes y vídeos** locales desde `%APPDATA%\koyun\fondos`.
- **Fondo en movimiento real**: el vídeo se reproduce en una ventana hija del `WorkerW` del escritorio, es decir, **detrás de los iconos**, y no intercepta el ratón.
- **Fondo distinto por monitor** usando la API `IDesktopWallpaper` de Windows. El selector detecta **todos** los monitores conectados (principal, secundarios, modelo y resolución) y permite fijar una imagen distinta en cada uno.
- **Miniaturas reales**: las imágenes se decodifican en Rust (crate `image`) y se sirven como `data:` URL, con caché en el frontend.
- **Rotación automática** de fondos con intervalo configurable.
- Etiqueta en cada miniatura indicando **en qué pantalla(s)** está puesto ese fondo.

### 🌐 Ajustes rápidos
Un cajón lateral dentro de la propia app (no abre los ajustes de Windows) con acciones **reales**:

| Sección | Qué hace |
|---|---|
| **Wi-Fi** | Lista las redes disponibles, muestra la señal, si tienen **perfil guardado** y si están conectadas. Un clic conecta (`netsh wlan connect name=`) y hay botón para desconectar. |
| **Bluetooth** | Lista dispositivos emparejados y emparejables, con botones **Conectar** y **Quitar**. |
| **Energía** | Muestra la **batería** (nivel y estado de carga) y permite cambiar el **plan de energía** (Equilibrado / Alto rendimiento / etc.). |

### 🔊 Controles
- Volumen (con barra arrastrable que no se interrumpe al hacer polling).
- Silenciar, elegir dispositivo de salida de audio.
- Radios: Wi-Fi, Bluetooth, modo avión.
- Brillo, notas adhesivas, ahorro de energía, modo «No molestar».
- **Corrección de un bug real**: aplicar un radio ya activo devolvía `0x8007139F` (`ERROR_INVALID_STATE`); ahora la operación es idempotente.

### ⚡ Energía y sistema
- Bloquear, suspender, reiniciar y apagar.
- Plans de energía, CPU/GPU/RAM en uso, temperatura y Media Session.

### 📋 Portapapeles
- Historial, restauración y borrado individual o completo.

### 🔘 Isla dinámica
- Una pastilla oscura tipo *notch* bajo la cámara que se expande y muestra el **logo de Koyun** al pasar el ratón.
- Al pulsarla abre/cierra el panel principal.

---

## Instalación

1. Descarga `koyun_0.1.0_x64-setup.exe` de la sección **Releases**.
2. Ejecútalo (instalador **NSIS**; se instala en `%LOCALAPPDATA%\koyun`).
3. Se añade un **icono en la bandeja del sistema** desde el que puedes mostrar, ocultar o salir.

> **Requisito:** Windows 10 o superior. En Windows 11 se aplica además el redondeo nativo de las esquinas de la ventana.

---

## Uso

| Acción | Cómo |
|---|---|
| Mostrar / ocultar el panel | `Ctrl+Alt+K`, clic en la isla, o clic en el icono de la bandeja |
| Cambiar de pestaña | `Ctrl+1` … `Ctrl+5` |
| Cerrar el panel | `Esc` (lo oculta, no termina la app) |
| Salir de la app | Menú del icono de bandeja → **Salir** |

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

## Cómo funciona por dentro

### Las tres ventanas

| Ventana | HTML | Propósito |
|---|---|---|
| `main` | `index.html` | El panel principal (780×520, sin decoraciones, siempre encima). |
| `island` | `island.html` | La isla dinámica (56×56, transparente, sin foco). Se muestra solo cuando el cursor entra en la zona superior del monitor. |
| `wallpaper` | `wallpaper.html` | Reproductor del vídeo de fondo. Se reparenta al `WorkerW` del escritorio. |

### Fondo en movimiento

`src-tauri/src/commands/videowallpaper.rs` usa el truco clásico de Windows:

1. Envía `0x052C` (`WM_USER`) al `Progman` para que cree el `WorkerW` de los iconos.
2. Localiza `SHELLDLL_DefView` y su `WorkerW` padre.
3. `SetParent` sobre la ventana `wallpaper` y `GetClientRect` + `SetWindowPos` para ajustarla al área del escritorio.
4. `set_ignore_cursor_events(true)` para que el vídeo no robe el ratón.

El estado se persiste en `%APPDATA%\koyun\fondo-animado.json` y se **reanuda automáticamente** al arrancar la app.

### Fondo por monitor

`src-tauri/src/commands/wallpaper.rs` instancia la interfaz COM `IDesktopWallpaper` para:

- `GetMonitorDevicePathCount()` → número real de monitores (soporta N).
- `GetMonitorDevicePathAt(i)` → identificador del monitor; de ahí se extrae el **modelo** (`AOC2243`, `CMN15E7`…).
- `GetMonitorRECT()` → resolución.
- `SetWallpaper(id, path)` → aplicar a una pantalla concreta.

### Redondeo de las esquinas

La ventana principal usa **redondeo nativo de Windows** (`DwmSetWindowAttribute` con `DWMWA_WINDOW_CORNER_PREFERENCE = DWMWCP_ROUND`), no transparencia. Esto evita el marco blanco que aparecía al activar `transparent: true` en el WebView2.

### Communication

Todo el acceso al sistema ocurre en **Rust** (crate `windows` 0.62). El frontend solo llama a comandos `invoke("nombre", { args })` registrados en `src-tauri/src/lib.rs`.

Los **errores** se registran en `%LOCALAPPDATA%\koyun\koyun.log` (con rotación por tamaño) para poder diagnosticar sin depurador.

---

## Estructura del proyecto

```
koyun/
├── index.html              # Ventana principal
├── island.html             # Isla dinámica
├── wallpaper.html          # Reproductor de fondo
├── vite.config.ts          # Entradas: index / island / wallpaper
├── src/
│   ├── main.tsx            # Bootstrap del panel
│   ├── App.tsx             # Layout: TopBar + pestañas
│   ├── styles.css          # Tailwind v4 + estilos globales
│   ├── components/
│   │   ├── TopBar.tsx
│   │   ├── QuickSettings.tsx      # Cajón de ajustes rápidos
│   │   ├── icons.tsx
│   │   └── tabs/                    # Controls / Power / Stats / Clipboard / Wallpaper
│   ├── island/main.tsx     # Lógica de la isla
│   ├── island/logo_isla.png
│   ├── wallpaper/main.tsx  # Reproductor de vídeo
│   ├── store/              # Zustand (ui, settings)
│   └── lib/                # Utilidades (polling, formato, errores, log)
└── src-tauri/
    ├── tauri.conf.json     # Ventanas, permisos, bundle NSIS
    ├── capabilities/default.json
    └── src/
        ├── lib.rs          # Builder, setup, atajo global, bandeja, comandos
        └── commands/
            ├── audio.rs        # Volumen y salidas de audio
            ├── clipboard.rs    # Historial del portapapeles
            ├── controls.rs     # Radios, tiles, brillo, powercfg
            ├── island.rs       # Isla dinámica y tracker del ratón
            ├── log.rs
            ├── media.rs        # Media Session
            ├── power.rs        # Bloquear / suspender / reiniciar / apagar
            ├── quick.rs        # Ajustes rápidos (Wi-Fi, BT, energía)
            ├── stats.rs        # CPU / GPU / RAM / temperatura
            ├── util.rs
            ├── videowallpaper.rs  # Fondo en movimiento
            └── wallpaper.rs       # Fondos, miniaturas y fondos por monitor
```

---

## Comandos Tauri

<details>
<summary>Ver todos</summary>

**Foncos**

- `wallpaper_list` — lista los fondos disponibles
- `wallpaper_thumbnail` — genera una miniatura `data:` URL
- `wallpaper_monitors` — devuelve los monitores detectados
- `wallpaper_set_monitor` — aplica un fondo a un monitor concreto
- `wallpaper_current` — devuelve el fondo activo
- `wallpaper_set` — aplica un fondo a todas las pantallas
- `wallpaper_import` / `wallpaper_delete` — importa o borra fondos
- `wallpaper_video_start` / `wallpaper_video_stop` / `wallpaper_video_state` — fondo en movimiento

**Ajustes rápidos**

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

## Solución de problemas

| Síntoma | Causa y solución |
|---|---|
| Una red aparece sin poder conectarse | Windows exige un **perfil guardado**. Koyun lo indica con la etiqueta «Sin perfil»: conéctala una vez desde Windows. |
| El vídeo de fondo no aparece | Ocurre al arrancar en Windows 11 multi‑monitor. Reinicia la app; el estado se reanuda solo. |
| El atajo `Ctrl+Alt+K` no responde | Otra aplicación puede tener registrado el atajo. Koyun avisa con un diálogo y sigue usando el icono de la bandeja. |
| El fondo de un monitor no cambia | Asegúrate de que la imagen esté en una ruta accesible y de que el monitor siga conectado. |
| Los avisos de «perfil» / emparejamiento tardan | Los dispositivos Bluetooth necesitan que la radio esté encendida (se puede desde el propio panel). |

Los errores técnicos quedan registrados en `%LOCALAPPDATA%\koyun\koyun.log`.

---

## Licencia

MIT

---

## Novedades recientes

### Mezclador por aplicación
- Controla el volumen y el silenciado por sesión de audio (aplicaciones) con actualización en tiempo real.
- Control del micrófono con silenciado directo.

### Historial de notificaciones
- Lista las notificaciones activas de Windows mediante `UserNotificationListener`.
- Acceso rápido a los ajustes de notificaciones del sistema.

### Ajustes unificados
- Apariencia: modo claro / oscuro / sistema, color de acento y paletas personalizables.
- Atajos globales configurables por acción (captura directa, deduplicado y validación).
- Opciones de isla dinámica y notificaciones.
- Todo se guarda en `%APPDATA%\koyun\config.json` con fusión profunda.

### Atajos globales configurables
- Los atajos fijos se migraron a configuración unificada.
- Registro dinámico desde Ajustes, sin recompilar.

### Modo Juego (seguro)
Pensado para priorizar el juego y limitar la carga secundaria sin requerir administrador:

- **Prioridad del juego**: eleva el proceso en primer plano a `HIGH_PRIORITY_CLASS` mediante `SetPriorityClass`
  y restaura la prioridad original al desactivar. No se usa `REALTIME_PRIORITY_CLASS`.
- **Vídeo de fondo**: pausa el `<video>` del escritorio (`WorkerW`) mientras el modo está activo.
- **Sondeos**: se detienen los sondeos del panel (estadísticas, audio, avisos) para liberar CPU.
- **Wi‑Fi**: desactiva el escaneo periódico (`netsh wlan set autoconfig enabled=no`) solo si estaba activo,
  guardando el estado y restaurándolo de forma síncrona al desactivar o cerrar la aplicación.
- **Restauración garantizada**: al desactivar, al cerrar desde la bandeja, al pedir cierre de ventana
  y desde el hook de pánico.
- No suspende procesos (`NtSuspendProcess`) ni purga el *working set*: se deja para una opción avanzada futura.
