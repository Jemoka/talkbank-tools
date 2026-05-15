import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./App.css";
import { DesktopRuntimeProvider } from "./runtime/DesktopRuntimeContext";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <DesktopRuntimeProvider>
      <App />
    </DesktopRuntimeProvider>
  </StrictMode>,
);
