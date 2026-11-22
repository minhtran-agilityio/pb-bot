import { useEffect, useState } from "react";

export type Theme = "system" | "dark" | "light";
const KEY = "shape-board.theme";
const ORDER: Theme[] = ["system", "dark", "light"];

function read(): Theme {
  try {
    const v = localStorage.getItem(KEY);
    return v === "dark" || v === "light" ? v : "system";
  } catch {
    return "system";
  }
}

/** System follows the OS (tokens.css media query); dark and light set data-theme on <html>. */
export function useTheme() {
  const [theme, setTheme] = useState<Theme>(read);
  useEffect(() => {
    const root = document.documentElement;
    if (theme === "system") root.removeAttribute("data-theme");
    else root.setAttribute("data-theme", theme);
    try {
      localStorage.setItem(KEY, theme);
    } catch {
      /* the theme still applies for this run */
    }
  }, [theme]);
  const cycle = () => setTheme((t) => ORDER[(ORDER.indexOf(t) + 1) % ORDER.length]);
  return { theme, cycle };
}
