import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { ScrapeProvider } from "./art/ScrapeProvider";
import { DownloadProvider } from "./browse/DownloadProvider";
import { ToastProvider } from "./components/Feedback/Feedback";
import { LibraryProvider } from "./data/LibraryProvider";
import { ThemeProvider } from "./theme/ThemeProvider";
import "./styles/fonts.css";
import "./styles/tokens.css";
import "./styles/global.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ThemeProvider>
      <ToastProvider>
        <LibraryProvider>
          <ScrapeProvider>
            <DownloadProvider>
              <App />
            </DownloadProvider>
          </ScrapeProvider>
        </LibraryProvider>
      </ToastProvider>
    </ThemeProvider>
  </StrictMode>,
);
