/* --------------------------------------------------------------------------
   Atajos globales

   Los accelerators se guardan como texto (`Ctrl+Alt+K`). Esta capa traduce
   los eventos de teclado a ese formato y comprueba que el backend acepte la
   combinación antes de guardarla.
   -------------------------------------------------------------------------- */

const MODIFIER_ORDER = ["ctrl", "alt", "shift", "meta"] as const;

const KEY_ALIASES: Record<string, string> = {
  control: "Ctrl",
  ctrl: "Ctrl",
  alt: "Alt",
  option: "Alt",
  shift: "Shift",
  meta: "Super",
  cmd: "Super",
  command: "Super",
  win: "Super",
  windows: "Super",
  esc: "Escape",
  escape: "Escape",
  enter: "Enter",
  return: "Enter",
  space: "Space",
  spacebar: "Space",
  tab: "Tab",
  backspace: "Backspace",
  delete: "Delete",
  del: "Delete",
  insert: "Insert",
  home: "Home",
  end: "End",
  pageup: "PageUp",
  pagedown: "PageDown",
  up: "Up",
  down: "Down",
  left: "Left",
  right: "Right",
  plus: "Plus",
  minus: "Minus",
  equal: "Equal",
};

const PUNCTUATION: Record<string, string> = {
  "`": "Backquote",
  "-": "Minus",
  "=": "Equal",
  "[": "BracketLeft",
  "]": "BracketRight",
  "\\": "Backslash",
  ";": "Semicolon",
  "'": "Quote",
  ",": "Comma",
  ".": "Period",
  "/": "Slash",
};

/** Convierte una tecla suelta al nombre que entiende `global-hotkey`. */
function keyName(event: KeyboardEvent): string {
  const key = event.key;

  if (PUNCTUATION[key]) return PUNCTUATION[key];

  if (/^F\d{1,2}$/i.test(key)) return key.toUpperCase();

  if (key.length === 1) {
    // Las letras y los números se expresan en mayúsculas («KeyA», «Digit1»).
    const upper = key.toUpperCase();
    if (/[A-Z]/.test(upper)) return `Key${upper}`;
    if (/[0-9]/.test(upper)) return `Digit${upper}`;
  }

  const alias = KEY_ALIASES[key.toLowerCase()];
  if (alias) return alias;

  return key;
}

/**
 * Construye el accelerator de un evento de teclado, o `null` si la pulsación
 * no describe una combinación válida (por ejemplo, solo una tecla suelta).
 */
export function acceleratorFromEvent(event: KeyboardEvent): string | null {
  const parts: string[] = [];
  if (event.ctrlKey) parts.push("Ctrl");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");
  if (event.metaKey) parts.push("Super");

  const key = keyName(event);

  // Una tecla modificadora por sí sola no es un atajo.
  const isModifier = ["Control", "Alt", "Shift", "Meta"].includes(key);
  if (isModifier) return null;

  // Sin modificadores solo se admiten teclas de función: evita capturar
  // teclas sueltas que el usuario usa para escribir.
  if (parts.length === 0 && !/^F\d{1,2}$/.test(key)) return null;

  parts.push(key);
  return parts.join("+");
}

/** Formatea un accelerator para mostrarlo al usuario. */
export function formatAccelerator(accelerator: string): string {
  if (!accelerator.trim()) return "Sin asignar";
  return accelerator
    .split("+")
    .map((part) => {
      const upper = part.trim().toUpperCase();
      if (upper === "CTRL") return "Ctrl";
      if (upper === "ALT") return "Alt";
      if (upper === "SHIFT") return "Shift";
      if (upper === "SUPER") return "Win";
      return part.trim();
    })
    .join(" + ");
}

export { MODIFIER_ORDER };
