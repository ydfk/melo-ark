import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import App from "@/App";
import { ThemeProvider } from "@/components/theme-provider";
import { Toaster } from "@/components/ui/sonner";

import "./index.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ThemeProvider defaultTheme="dark" storageKey="meloark-theme">
      <App />
      <Toaster />
    </ThemeProvider>
  </StrictMode>
);
