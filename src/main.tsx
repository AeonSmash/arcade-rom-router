import React from "react";
import ReactDOM from "react-dom/client";
import { isTauri } from "@tauri-apps/api/core";

import App from "./app/App";
import { DesktopOnlyNotice } from "./components/DesktopOnlyNotice";
import "./styles/global.css";

const container = document.getElementById("root");

if (!container) {
  throw new Error("Root element is missing from index.html");
}

// Decided here rather than inside `App` so that none of its hooks, which all
// assume a backend, ever run in a plain browser tab.
ReactDOM.createRoot(container).render(
  <React.StrictMode>
    {isTauri() ? <App /> : <DesktopOnlyNotice />}
  </React.StrictMode>
);
