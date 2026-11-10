import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { findCommands } from "./commands";
import { useLauncherWindow } from "./hooks/useLauncherWindow";
import { executeCommand } from "./services/commandService";
import { useAppCatalog } from "./hooks/useAppCatalog";
import { indexApplications, indexCommand, moveSelection, rankApplications, RESULT_BATCH_SIZE, visibleResultCount, type SearchItem } from "./appSearch";
import { parseLaunchCounts, recordSuccessfulLaunch, USAGE_STORAGE_KEY } from "./launchUsage";
import { installedAppActions } from "./services/installedAppService";
import { AppIcon, makeIconLoader } from "./components/AppIcon";
import "./App.css";

function App() {
  const inputRef = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [message, setMessage] = useState("");
  const [commandError, setCommandError] = useState("");
  const [isRunning, setIsRunning] = useState(false);
  const runningRef = useRef(false);
  const resultsRef = useRef<HTMLElement>(null);
  const optionRefs = useRef(new Map<string, HTMLLIElement>());
  const [visibleCount, setVisibleCount] = useState(RESULT_BATCH_SIZE);
  const [usageNotice, setUsageNotice] = useState("");
  const [launchCounts, setLaunchCounts] = useState(() => {
    try { return parseLaunchCounts(localStorage.getItem(USAGE_STORAGE_KEY)); }
    catch { return {}; }
  });
  const { hideWindow, startDrag, windowError, shortcutNotice } = useLauncherWindow(inputRef);
  const { catalog, loading, error: catalogError, refreshCatalog } = useAppCatalog();
  const loadIcon = useMemo(makeIconLoader, [catalog?.generation]);
  const appIndex = useMemo(() => indexApplications(catalog?.apps ?? []), [catalog]);
  const results = useMemo(() => rankApplications([
    ...findCommands(query).map(indexCommand), ...appIndex,
  ], query, launchCounts), [query, appIndex, launchCounts]);
  const activeIndex = Math.min(selectedIndex, Math.max(0, results.length - 1));
  const selected = results[activeIndex];
  const shownCount = visibleResultCount(visibleCount, activeIndex, results.length);

  useEffect(() => {
    setSelectedIndex(0);
    setVisibleCount(RESULT_BATCH_SIZE);
    resultsRef.current?.scrollTo({ top: 0 });
  }, [query, catalog]);

  useEffect(() => {
    if (selected) optionRefs.current.get(selected.key)?.scrollIntoView({ block: "nearest" });
  }, [selected?.key]);

  function activate(result: SearchItem) {
    void runResult(result);
  }

  async function runResult(result: SearchItem) {
    // Ref closes the gap before React rerenders, preventing double click/key launches.
    if (runningRef.current) return;
    runningRef.current = true;
    setIsRunning(true);
    setCommandError("");
    setMessage("Opening " + result.label + "…");
    try {
      if (result.kind === "command") await executeCommand(result.command.id);
      else {
        const outcome = await installedAppActions.launch(result.app.id);
        if (!outcome.launched) throw new Error("Windows did not confirm opening the application.");
        if (outcome.warning) setCommandError(outcome.warning);
      }
      const nextCounts = recordSuccessfulLaunch(launchCounts, result.key);
      setLaunchCounts(nextCounts);
      try {
        localStorage.setItem(USAGE_STORAGE_KEY, JSON.stringify(nextCounts));
        setUsageNotice("");
      } catch { setUsageNotice("Launch history is available for this session only."); }
      setQuery("");
      setSelectedIndex(0);
    } catch (error) {
      setCommandError(typeof error === "string" ? error : "Could not open " + result.label + ". " + String(error));
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
      const next = moveSelection(activeIndex, direction, results.length);
      setSelectedIndex(next);
      setVisibleCount(visibleResultCount(shownCount, next, results.length));
    } else if (event.key === "Enter") {
      event.preventDefault();
      if (selected && !event.repeat) activate(selected);
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
          aria-label="Search commands and applications"
          aria-autocomplete="list"
          aria-expanded={results.length > 0}
          aria-controls="command-results"
          aria-activedescendant={selected ? "result-" + activeIndex : undefined}
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
      <section ref={resultsRef} className="results" aria-label="Search results" onScroll={(event) => {
        const panel = event.currentTarget;
        if (panel.scrollTop + panel.clientHeight >= panel.scrollHeight - 24) {
          setVisibleCount((count) => Math.min(results.length, Math.max(count, shownCount) + RESULT_BATCH_SIZE));
        }
      }}>
        <div className="section-label">{query.trim() ? "RESULTS" : "SUGGESTED"}<span>{results.length}</span>
          <button className="refresh-button" disabled={loading} onClick={() => { void refreshCatalog(); inputRef.current?.focus(); }}>
            {loading ? (catalog ? "Refreshing…" : "Loading apps…") : catalogError ? "Retry apps" : "Refresh apps"}
          </button>
        </div>
        <div className="catalog-status" role="status">
          {loading && <p>{catalog ? "Refreshing applications. Current results remain available." : "Loading installed applications… Commands are ready."}</p>}
          {catalogError && <p className="error">{catalogError} {catalog ? "Showing the previous catalog." : "Commands still work."}</p>}
          {!loading && catalog?.apps.length === 0 && <p>No installed applications were found. Commands are still available.</p>}
          {catalog && (catalog.warnings.length > 0 || catalog.sources.some((source) => !source.completed)) && <p>Some applications could not be read. Showing available results; use Refresh apps to retry.</p>}
        </div>
        <ul id="command-results" role="listbox" aria-label="Commands and applications" aria-busy={isRunning}>
          {results.slice(0, shownCount).map((result, index) => (
            <li
              id={"result-" + index}
              key={result.key}
              ref={(element) => { if (element) optionRefs.current.set(result.key, element); else optionRefs.current.delete(result.key); }}
              role="option"
              aria-selected={index === activeIndex}
              aria-disabled={isRunning}
              aria-posinset={index + 1}
              aria-setsize={results.length}
              className={"command-result" + (index === activeIndex ? " selected" : "")}
              onMouseDown={(event) => event.preventDefault()}
              onMouseMove={() => setSelectedIndex(index)}
              onClick={() => { if (!isRunning) activate(result); }}
            >
              <AppIcon appId={result.kind === "app" ? result.app.id : undefined} load={loadIcon} />
              <span className="command-copy"><span className="command-title" title={result.label}>{result.label}</span><span className="command-description">{result.kind === "command" ? result.command.description : result.app.launch.method === "packaged_app" ? "Windows application" : "Installed application"}</span></span>
              <span className="command-badge">{isRunning ? "Opening…" : result.kind === "app" ? "Application" : "Command"}</span>
              <kbd className="enter-key" aria-hidden="true">↵</kbd>
            </li>
          ))}
        </ul>
        {shownCount < results.length && <button className="more-results" onClick={() => { setVisibleCount(shownCount + RESULT_BATCH_SIZE); inputRef.current?.focus(); }}>Show more ({results.length - shownCount} remaining)</button>}
        {results.length === 0 && !loading && <p className="empty-state">No matching commands or applications. Try <strong>open notepad</strong>.</p>}
      </section>
      <p className={"status" + (commandError || windowError || shortcutNotice ? " error" : "")} role="status">{commandError || windowError || message || shortcutNotice || usageNotice}</p>
      <footer className="footer">
        <span className="keyboard-hints"><span><kbd>↑</kbd><kbd>↓</kbd> navigate</span><span><kbd>↵</kbd> open</span></span>
        <span>Ctrl + Shift + Space</span>
      </footer>
    </main>
  );
}

export default App;
