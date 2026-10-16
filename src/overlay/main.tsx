import React from "react";
import ReactDOM from "react-dom/client";
import { Overlay } from "@/overlay/Overlay";
import "@/index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Overlay />
  </React.StrictMode>,
);
