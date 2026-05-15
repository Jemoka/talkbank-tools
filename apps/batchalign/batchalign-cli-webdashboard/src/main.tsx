/** Browser entry point for the shared dashboard frontend.
 *
 * The dashboard bootstraps React Query first, then provides the explicit
 * desktop runtime context so every component and hook shares the same
 * desktop-vs-web boundary.
 */

import { createRoot } from "react-dom/client";
import { QueryClientProvider } from "@tanstack/react-query";
import { App } from "./app";
import { DesktopProvider } from "./desktop/DesktopContext";
import { queryClient } from "./query";
import "./app.css";

createRoot(document.getElementById("app")!).render(
  <QueryClientProvider client={queryClient}>
    <DesktopProvider>
      <App />
    </DesktopProvider>
  </QueryClientProvider>
);
