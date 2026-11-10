import { useRef, useState, type KeyboardEvent } from "react";
import { findCommands, type Command } from "./commands";
import { useLauncherWindow } from "./hooks/useLauncherWindow";
import { executeCommand } from "./services/commandService";
import "./App.css";

function App() {
  const inputRef = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [message, setMessage] = useState("");
  const [commandError, setCommandError] = useState("");
  const [isRunning, setIsRunning] = useState(false);
  const runningRef = useRef(false);
  const { hideWindow, startDrag, windowError, shortcutNotice } = useLauncherWindow(inputRef);
  const results = findCommands(query);
  const selected = results[selectedIndex];

  async function runCommand(command: Command) {
    // Ref closes the gap before React rerenders, preventing double click/key launches.
    if (runningRef.current) return;
    runningRef.current = true;
    setIsRunning(true);
    setCommandError("");
    setMessage("Running " + command.label + "…");
    try {
      await executeCommand(command.id);
      setQuery("");
      setSelectedIndex(0);
    } catch (error) {
      setCommandError(typeof error === "string" ? error : "Could not run " + command.label + ". " + String(error));
    } finally {
      runningRef.current = false;
      setIsRunning(false);
      setMessage("");
      inputRef.current?.focus();
    }
  }

  function handleKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.nativeEvent.isComposing) return;
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      if (results.length === 0) return;
      const direction = event.key === "ArrowDown" ? 1 : -1;
      setSelectedIndex((index) => (index + direction + results.length) % results.length);
    } else if (event.key === "Enter") {
      event.preventDefault();
      if (selected && !event.repeat) void runCommand(selected);
    }
  }

  return (
    <main className="launcher" aria-label="NEXUS command bar">
      <header
        className="brand-bar"
        title="Drag to move NEXUS"
        onMouseDown={(event) => {
          if (event.button === 0) {
            event.preventDefault();
            void startDrag();
          }
        }}
      >
        <div className="brand"><span className="brand-mark" aria-hidden="true">N</span>NEXUS</div>
        <span className="local-badge"><span aria-hidden="true" />LOCAL &amp; READY</span>
      </header>
      <div className="search-field">
        <svg className="search-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" aria-hidden="true">
          <circle cx="10.5" cy="10.5" r="6.5" /><path d="m16 16 4.5 4.5" />
        </svg>
        <input
          ref={inputRef}
          autoFocus
          role="combobox"
          aria-label="Search commands"
          aria-autocomplete="list"
          aria-expanded={results.length > 0}
          aria-controls="command-results"
          aria-activedescendant={selected ? "command-" + selected.id : undefined}
          autoComplete="off"
          spellCheck={false}
          readOnly={isRunning}
          placeholder="What would you like to do?"
          value={query}
          onChange={(event) => {
            setQuery(event.currentTarget.value);
            setSelectedIndex(0);
            setMessage("");
            setCommandError("");
          }}
          onKeyDown={handleKeyDown}
        />
        <button className="escape-button" onClick={() => void hideWindow()} aria-label="Hide NEXUS">esc</button>
      </div>
      <section className="results" aria-label="Command results">
        <div className="section-label">{query.trim() ? "RESULTS" : "SUGGESTED"}<span>{results.length}</span></div>
        <ul id="command-results" role="listbox" aria-label="Commands" aria-busy={isRunning}>
          {results.map((command, index) => (
            <li
              id={"command-" + command.id}
              key={command.id}
              role="option"
              aria-selected={index === selectedIndex}
              aria-disabled={isRunning}
              className={"command-result" + (index === selectedIndex ? " selected" : "")}
              onMouseDown={(event) => event.preventDefault()}
              onMouseMove={() => setSelectedIndex(index)}
              onClick={() => void runCommand(command)}
            >
              <span className="command-icon" aria-hidden="true">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                  <rect x="5" y="3.5" width="14" height="17" rx="2" /><path d="M9 8h6M9 12h6M9 16h4" />
                </svg>
              </span>
              <span className="command-copy"><span className="command-title">{command.label}</span><span className="command-description">{command.description}</span></span>
              <span className="command-badge">{isRunning ? "Opening…" : "Application"}</span>
              <kbd className="enter-key" aria-hidden="true">↵</kbd>
            </li>
          ))}
        </ul>
        {results.length === 0 && <p className="empty-state">No matching commands. Try <strong>open notepad</strong>.</p>}
        <p className={"status" + (commandError || windowError || shortcutNotice ? " error" : "")} role="status">{commandError || windowError || message || shortcutNotice}</p>
      </section>
      <footer className="footer">
        <span className="keyboard-hints"><span><kbd>↑</kbd><kbd>↓</kbd> navigate</span><span><kbd>↵</kbd> open</span></span>
        <span>Ctrl + Shift + Space</span>
      </footer>
    </main>
  );
}

export default App;
