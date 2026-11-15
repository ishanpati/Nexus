import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { AudioFeedback } from "./components/AudioFeedback";
import { WorkflowStudio } from "./components/WorkflowStudio";

const audioFeedback = new URLSearchParams(window.location.search).has("audio-feedback");
if (audioFeedback) document.body.classList.add("audio-overlay-body");
const studio = new URLSearchParams(window.location.search).has("workflow-studio");
if (studio) document.body.classList.add("studio-body");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {audioFeedback ? <AudioFeedback /> : studio ? <WorkflowStudio /> : <App />}
  </React.StrictMode>,
);
