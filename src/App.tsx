import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { findCommands } from "./commands";
import { useLauncherWindow } from "./hooks/useLauncherWindow";
import { executeCommand } from "./services/commandService";
import { useAppCatalog } from "./hooks/useAppCatalog";
import { indexApplications, indexCommand, moveSelection, rankApplications, RESULT_BATCH_SIZE, visibleResultCount, type SearchItem } from "./appSearch";
import { parseLaunchCounts, recordSuccessfulLaunch, USAGE_STORAGE_KEY } from "./launchUsage";
import { installedAppActions } from "./services/installedAppService";
import { AppIcon, makeIconLoader } from "./components/AppIcon";
import { useFileSearch } from "./hooks/useFileSearch";
import { FILE_BATCH_SIZE, FILE_RESULT_LIMIT, searchMode, type FileResult } from "./fileSearch";
import { FileResultContent } from "./components/FileResultContent";
import { invoke } from "@tauri-apps/api/core";
import { canActivateFile, createFileActions, type FileAction } from "./fileActions";
import { FileConfirmation } from "./components/FileConfirmation";
import { recordFileOpen } from "./fileUsage";
import "./App.css";

type ResultItem = SearchItem | { kind: "file"; key: string; label: string; file: FileResult };
const openFile = createFileActions(invoke);

function App() {
  const inputRef = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [message, setMessage] = useState("");
  const [commandError, setCommandError] = useState("");
  const [isRunning, setIsRunning] = useState(false);
  const [confirmation, setConfirmation] = useState<{ path: string; message: string } | null>(null);
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
  const mode = searchMode(query);
  const fileMode = mode.kind === "files";
  const fileQuery = mode.kind === "files" ? mode.query : null;
  const files = useFileSearch(fileQuery);
  const fileLoading = !!fileQuery && (files.loading || files.query !== fileQuery);
  const batchSize = fileMode ? FILE_BATCH_SIZE : RESULT_BATCH_SIZE;
  const appResults = useMemo(() => fileMode ? [] : rankApplications([
    ...findCommands(query).map(indexCommand), ...appIndex,
  ], query, launchCounts), [query, appIndex, launchCounts, fileMode]);
  const fileResults: readonly ResultItem[] = useMemo(() => files.results.map((file) => ({
    kind: "file" as const, key: "file:" + file.full_path, label: file.filename, file,
  })), [files.results]);
  const results: readonly ResultItem[] = fileMode ? (fileQuery ? fileResults : []) : appResults;
  const activeIndex = Math.min(selectedIndex, Math.max(0, results.length - 1));
  const selected = results[activeIndex];
  const shownCount = fileMode
    ? Math.min(results.length, Math.max(visibleCount, Math.ceil((activeIndex + 1) / FILE_BATCH_SIZE) * FILE_BATCH_SIZE))
    : visibleResultCount(visibleCount, activeIndex, results.length);

  useEffect(() => {
    setSelectedIndex(0);
    setVisibleCount(batchSize);
    resultsRef.current?.scrollTo({ top: 0 });
  }, [query, fileMode ? files.results : catalog, batchSize]);

  useEffect(() => {
    if (selected) optionRefs.current.get(selected.key)?.scrollIntoView({ block: "nearest" });
  }, [selected?.key]);

  function activate(result: ResultItem) {
    if (result.kind === "file") {
      if (!canActivateFile(fileLoading, files.resultQuery, fileQuery)) return;
      void runFile(result.file, "open");
      return;
    }
    void runResult(result);
  }

  async function runFile(file: Pick<FileResult, "full_path">, action: FileAction, confirmed = false) {
    if (runningRef.current) return;
    runningRef.current = true;
    setIsRunning(true);
    setCommandError("");
    setMessage(action === "reveal" ? "Showing item in File Explorer…" : "Opening item…");
    try {
      const outcome = await openFile(file, action, confirmed);
      if (outcome.status === "confirmation_required") setConfirmation(outcome);
      else {
        if (action === "open") setUsageNotice(recordFileOpen(file.full_path) ?? "");
        setConfirmation(null);
        setQuery("");
        setSelectedIndex(0);
        if (outcome.warning) setCommandError(outcome.warning);
      }
    } catch (error) {
      setCommandError(typeof error === "string" ? error : "Could not open this item. " + String(error));
      inputRef.current?.focus();
    } finally {
      runningRef.current = false;
      setIsRunning(false);
      setMessage("");
    }
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
      setVisibleCount(fileMode ? Math.min(results.length, Math.max(shownCount, Math.ceil((next + 1) / FILE_BATCH_SIZE) * FILE_BATCH_SIZE)) : visibleResultCount(shownCount, next, results.length));
    } else if (event.key === "Enter") {
      event.preventDefault();
      if (selected && !event.repeat) {
        if (event.ctrlKey && selected.kind === "file" && canActivateFile(fileLoading, files.resultQuery, fileQuery)) void runFile(selected.file, "reveal");
        else activate(selected);
      }
    }
  }

  return (
    <main className="launcher" aria-label="NEXUS command bar">
      {confirmation && <FileConfirmation path={confirmation.path} message={confirmation.message}
        onCancel={() => { setConfirmation(null); inputRef.current?.focus(); }}
        onConfirm={() => { const path = confirmation.path; setConfirmation(null); void runFile({ full_path: path }, "open", true); }} />}
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
      <div className={"search-field" + (fileMode ? " file-mode" : "")}>
        <svg className="search-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" aria-hidden="true">
          <circle cx="10.5" cy="10.5" r="6.5" /><path d="m16 16 4.5 4.5" />
        </svg>
        <input
          ref={inputRef}
          autoFocus
          role="combobox"
          aria-label={fileMode ? "Search files and folders" : "Search commands and applications"}
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
          setVisibleCount((count) => Math.min(results.length, Math.max(count, shownCount) + batchSize));
        }
      }}>
        <div className="section-label">{fileMode ? "FILES & FOLDERS" : query.trim() ? "RESULTS" : "SUGGESTED"}<span>{results.length}{fileMode && results.length === FILE_RESULT_LIMIT ? " (limit)" : ""}</span>
          {fileMode ? <button className="refresh-button" disabled={!fileQuery || fileLoading} onClick={() => { files.retry(); inputRef.current?.focus(); }}>Retry search</button> : <button className="refresh-button" disabled={loading} onClick={() => { void refreshCatalog(); inputRef.current?.focus(); }}>
            {loading ? (catalog ? "Refreshing…" : "Loading apps…") : catalogError ? "Retry apps" : "Refresh apps"}
          </button>}
        </div>
        {fileMode ? <div className="catalog-status file-search-status" role="status">
          {!fileQuery ? "Type a filename or folder name after f ." : fileLoading
            ? (results.length ? "Updating… Previous matches remain visible." : "Searching files and folders…")
            : files.error ? <span className="error">{files.error}</span>
            : results.length === 0 ? "No matching files or folders." : "Enter to open · Ctrl+Enter to show in folder"}
        </div> : <div className="catalog-status" role="status">
          {loading && <p>{catalog ? "Refreshing applications. Current results remain available." : "Loading installed applications… Commands are ready."}</p>}
          {catalogError && <p className="error">{catalogError} {catalog ? "Showing the previous catalog." : "Commands still work."}</p>}
          {!loading && catalog?.apps.length === 0 && <p>No installed applications were found. Commands are still available.</p>}
          {catalog && (catalog.warnings.length > 0 || catalog.sources.some((source) => !source.completed)) && <p>Some applications could not be read. Showing available results; use Refresh apps to retry.</p>}
        </div>}
        <ul id="command-results" role="listbox" aria-label={fileMode ? "Files and folders" : "Commands and applications"} aria-busy={isRunning || (fileMode && fileLoading)}>
          {results.slice(0, shownCount).map((result, index) => (
            <li
              id={"result-" + index}
              key={result.key}
              ref={(element) => { if (element) optionRefs.current.set(result.key, element); else optionRefs.current.delete(result.key); }}
              role="option"
              aria-selected={index === activeIndex}
              aria-disabled={isRunning || (fileMode && fileLoading)}
              aria-posinset={index + 1}
              aria-setsize={results.length}
              className={"command-result" + (index === activeIndex ? " selected" : "")}
              onMouseDown={(event) => event.preventDefault()}
              onMouseMove={() => setSelectedIndex(index)}
              onClick={() => { if (!isRunning) activate(result); }}
            >
              {result.kind === "file" ? <><FileResultContent file={result.file} query={files.resultQuery} />
                <button className="reveal-button" title="Show in containing folder (Ctrl+Enter)" aria-label={"Show " + result.label + " in containing folder"}
                  disabled={isRunning || fileLoading} onClick={(event) => { event.stopPropagation(); if (canActivateFile(fileLoading, files.resultQuery, fileQuery)) void runFile(result.file, "reveal"); }}>Show</button></> : <>
              <AppIcon appId={result.kind === "app" ? result.app.id : undefined} load={loadIcon} />
              <span className="command-copy"><span className="command-title" title={result.label}>{result.label}</span><span className="command-description">{result.kind === "command" ? result.command.description : result.app.launch.method === "packaged_app" ? "Windows application" : "Installed application"}</span></span>
              <span className="command-badge">{isRunning ? "Opening…" : result.kind === "app" ? "Application" : "Command"}</span>
              </>}
              <kbd className="enter-key" aria-hidden="true">↵</kbd>
            </li>
          ))}
        </ul>
        {shownCount < results.length && <button className="more-results" onClick={() => { setVisibleCount(shownCount + batchSize); inputRef.current?.focus(); }}>Show more ({results.length - shownCount} remaining)</button>}
        {!fileMode && results.length === 0 && !loading && <p className="empty-state">No matching commands or applications. Try <strong>open notepad</strong>.</p>}
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
