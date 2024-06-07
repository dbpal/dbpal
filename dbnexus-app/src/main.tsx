import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";
import "./comp/Dashboard"
import Dashboard from "./comp/Dashboard";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Dashboard></Dashboard>
  </React.StrictMode>,
);
