import { useEffect, useRef } from "react";
import { confirmationGate } from "../services/systemActionService";

export function SleepConfirmation({ message, onConfirm, onCancel }: {
  message: string; onConfirm: () => void; onCancel: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const once = useRef(confirmationGate());
  useEffect(() => { dialog.current?.showModal(); }, []);
  return <dialog ref={dialog} className="file-confirmation" aria-labelledby="sleep-confirm-title"
    onCancel={() => once.current(onCancel)} onKeyDown={(event) => {
      if (event.repeat && (event.key === "Enter" || event.key === " ")) event.preventDefault();
      if (event.key === "Escape" && !event.nativeEvent.isComposing) once.current(onCancel);
    }}>
    <strong id="sleep-confirm-title">Put this computer to sleep?</strong>
    <p>{message}</p>
    <div><button autoFocus onClick={() => once.current(onCancel)}>Cancel</button>
      <button onClick={() => once.current(onConfirm)}>Sleep now</button></div>
  </dialog>;
}
