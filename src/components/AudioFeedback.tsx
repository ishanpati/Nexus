import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import "./AudioFeedback.css";

type Snapshot = { volume: number; muted: boolean };
export function AudioFeedback() {
  const [state, setState] = useState<Snapshot | null>(null);
  useEffect(() => {
    let disposed = false;
    let received = false;
    const subscription = listen<Snapshot>("audio-feedback", ({ payload }) => {
      received = true;
      if (!disposed) setState(payload);
    });
    void subscription.then(() => invoke<Snapshot | null>("audio_feedback_state")).then((snapshot) => {
      if (!disposed && !received) setState(snapshot);
    }).catch(() => {}); // Feedback is optional; native audio error reporting is separate.
    return () => { disposed = true; void subscription.then(unlisten => unlisten()).catch(() => {}); };
  }, []);
  return <div className="audio-feedback" role="status" aria-live="polite">
    <span className="audio-feedback-brand">NEXUS</span>
    <span>{state ? (state.muted ? "Muted" : "Volume") : "Audio"}</span>
    <strong>{state?.volume ?? 0}%</strong>
    <progress aria-label="Output volume" value={state?.volume ?? 0} max={100} />
  </div>;
}
