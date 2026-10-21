import { Component, type ErrorInfo, type ReactNode } from "react";

interface Props {
  children: ReactNode;
}

interface State {
  error: Error | null;
}

/**
 * Sin un límite de errores, cualquier excepción durante el render deja la
 * webview en blanco de forma permanente: Tauri no recarga la ventana y el
 * usuario pierde la sesión sin ningún mensaje.
 */
export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("[Koyun] error de interfaz", error, info.componentStack);
  }

  private retry = () => {
    this.setState({ error: null });
  };

  render() {
    if (this.state.error) {
      return (
        <div className="flex h-full flex-col items-center justify-center gap-3 px-8 text-center">
          <p className="text-sm font-medium text-slate-100">La interfaz encontró un error</p>
          <p className="max-w-full break-words text-xs leading-5 text-white/50">
            {this.state.error.message}
          </p>
          <button
            type="button"
            onClick={this.retry}
            className="rounded-md border border-white/10 bg-white/5 px-3 py-1.5 text-xs text-slate-200 transition-colors hover:border-cyan-400/40 hover:text-cyan-200"
          >
            Reintentar
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
