import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./app/App";
import { AppProviders } from "./app/providers";
import { applyUiPreferences } from "./app/uiPreferences";
import { useUiStore } from "./store/uiStore";
import "./styles/globals.css";

applyUiPreferences(useUiStore.getState());

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <AppProviders>
      <App />
    </AppProviders>
  </React.StrictMode>,
);

