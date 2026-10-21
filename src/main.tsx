import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { ErrorBoundary } from "./components/ErrorBoundary";
import "./styles.css";

// Un rechazo de promesa no renderizado por React quedaría en silencio total
// dentro de un empaquetado sin consola visible.
window.addEventListener("unhandledrejection", (event) => {
  console.error("[Koyun] promesa rechazada", event.reason);
});

window.addEventListener("error", (event) => {
  console.error("[Koyun] error no capturado", event.error ?? event.message);
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </React.StrictMode>,
);
