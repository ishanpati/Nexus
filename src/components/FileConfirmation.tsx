import { useEffect, useRef } from "react";

export function FileConfirmation({ path, message, onConfirm, onCancel }: {
  path: string; message: string; onConfirm: () => void; onCancel: () => void;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => { ref.current?.showModal(); }, []);
  return <dialog ref={ref} className="file-confirmation" aria-labelledby="file-confirm-title"
    onCancel={onCancel} onKeyDown={(event) => {
      if (event.key === "Enter" && event.repeat) event.preventDefault();
      // The window-level Escape handler hides NEXUS and prevents native cancel.
      // Clear this prompt too, while letting the event reach that handler.
      if (event.key === "Escape" && !event.nativeEvent.isComposing) onCancel();
    }}>
    <strong id="file-confirm-title">Open potentially executable file?</strong>
    <p>{message}</p><p className="confirm-path" title={path}>{path}</p>
    <div><button autoFocus onClick={onCancel}>Cancel</button><button onClick={onConfirm}>Open this file</button></div>
  </dialog>;
}
