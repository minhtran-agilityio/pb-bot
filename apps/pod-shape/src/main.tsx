import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./styles/globals.css";

// A file dropped outside a DropZone must not navigate the window to file:// (design.md §8, advisor
// item 3). The DropZone handles its own drop events; everything else is swallowed here. The
// navigation guard in nav.rs is the second layer.
for (const type of ["dragover", "drop"] as const) {
  window.addEventListener(type, (e) => e.preventDefault());
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
