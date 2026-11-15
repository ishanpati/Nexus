import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { AudioFeedback } from "./components/AudioFeedback";

const audioFeedback = new URLSearchParams(window.location.search).has("audio-feedback");
if (audioFeedback) document.body.classList.add("audio-overlay-body");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {audioFeedback ? <AudioFeedback /> : <App />}
  </React.StrictMode>,
);
