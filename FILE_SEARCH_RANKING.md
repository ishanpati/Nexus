# File-search quality: v0.3.1 working branch

Branch verified before edits: `fix/file-search-ranking`; the working tree was clean.
No master changes, package-version bump, release bundle, dependency, permission,
application-launch handler, shortcut, or window-style changes are part of this task.

## Diagnosis verified against installed ES

The previous backend always used `-match-path`, applied `-n` before any NEXUS
ranking, and returned ES order directly. Files and directories shared that one pool.
The UI displayed the first ten of up to 100 received items; it did not rank files.

With ES **1.1.0.38**, a live full-path `nexus` search returned `.git`, `node_modules`,
and `target` descendants first. A filename-only `/ad` exact regex immediately found
`C:\Users\ishan\Projects\nexus`. Directory attributes parsed correctly: crowding,
not broken directory classification, caused this reproduced case.

The finance tracker Desktop copy is actually under `OneDrive\Desktop`. Exact-stem
queries also found an IPCH cache and Recent shortcut. There was no location policy
or native Known Folder resolution to prefer Desktop.

Local `es.exe -help` and live calls verified `/ad`, `/a-d`, `-no-match-path`,
`-match-path`, anchored regex, Unicode hex literals, `-sort path`, EFU output, and
`-no-folder-append-path-separator`. See the [official ES CLI reference](https://www.voidtools.com/en-us/support/everything/command_line_interface/).

## Retrieval and ranking

Normal queries run these phases in order:

1. Exact directory name; exact filename or last-extension stem.
2. Name prefix.
3. Name substring (word-boundary candidates are ranked above other substrings).
4. Full path, only if name candidates are insufficient to fill the requested limit.

Every phase has separate folder/file pools of **128 each**, regardless of the
requested display limit. The two queries run concurrently. After merging and
deduplicating a phase, lower-quality phases are skipped if enough results exist.
Thus each search runs **2–8 ES processes total, at most two concurrently**, collecting
at most 1,024 raw rows; explicit path queries use only the two path pools. The
deadline is five seconds for the entire retrieval, not five seconds per pass.
Each child retains the existing output bounds and kill/reap behavior. Any failed
pass produces an error rather than disguising failure as an empty catalog.

All user text is hex-escaped literal regex data. Only NEXUS supplies regex anchors
and the optional final-extension pattern. No Everything expression injection,
shell command execution, drive scan, or fuzzy index is introduced.

Sort order is lexicographic: **match tier**, descending contextual score, folded
filename, normalized full path, then original path. Tiers are exact name/stem,
prefix, word boundary, substring, and path-only. Fuzzy tier is deliberately unused
because no bounded fuzzy source is implemented. Context cannot cross match tiers.

Context within a tier:

- Direct Desktop: +12; direct Documents/Downloads: +8, resolved through
  `SHGetKnownFolderPath` (including OneDrive redirection).
- At/in a configured project root: +4. `NEXUS_PROJECT_DIRS` is a Windows path list,
  for example `C:\Users\ishan\Projects;D:\work`. Only absolute roots are used, up
  to 32. Restart NEXUS after changes. Missing Known Folders simply omit that bonus.
- Previously opened in NEXUS: +2, independent of frequency. The latest 256 file
  opens are kept in local storage, separate from app usage. Reveal, failed opens,
  confirmation requests, and cancellations do not record history. Storage failure
  retains session history and reports a notice.
- Temp/cache/CrashDumps/IPCH: -12 per directory component; `node_modules`/`.git`:
  -16; `dist`/`target`/`build`/`out`: -8. Aggregate score is floored at -48.
  These are penalties, never exclusion rules.

Deduplication normalizes case, slash direction, extended path prefixes and trailing
separators. Different full paths with the same name remain separate. Final 1–100
result truncation happens after ranking. Native file classification remains based
on EFU attributes and is checked against each pass's file/folder filter.

The frontend still uses its 150 ms debounce and latest-generation guard. Old
successes and failures cannot replace new results or re-enter file mode. It does
not kill an in-flight old search; it keeps only the newest pending query, which
waits for that bounded operation to finish. Results remain non-activatable while
updating. Existing v0.2 app matching and launching are unchanged.

## Verification and limitations

- Frontend: **42 tests passed**, TypeScript and Vite production build passed.
- Rust: **59 tests passed**, one intentionally ignored subprocess fixture is
  exercised by the passing timeout test; native executable and search example built.
- Regression tests include exact folder versus descendant paths, Desktop versus
  Temp `.xlsx`, stems, tier dominance, context/history, Unicode, duplicate paths,
  stable order, empty/no-match handling, pass limits, errors, stale responses, and
  the existing application matching/launching tests.
- Live backend: `nexus` now returns the project directory first;
  `finance_tracker` returns the OneDrive Desktop executable first. The real local
  file is `.exe`; the requested `.xlsx` case is covered by automated fixtures.
  No discovered file was opened by these diagnostics.
- Live Unicode/spaces/punctuation queries found the existing fixture names.
  A unique no-match query returned an empty array successfully.

Measured debug diagnostic wall times include process startup, ES IPC and JSON;
they exclude the UI's 150 ms debounce. With limit 10, initial samples were 415 ms
for `nexus`, 747 ms for `finance_tracker`, and about 1.9–2.2 s for sparse/no-match
queries requiring all passes. With the UI limit of 100, three `nexus` samples were
648/608/652 ms and three finance samples were 1886/1890/1931 ms. These are local
samples, not an optimized-build performance guarantee.

Finite pools cannot guarantee that every equally named copy is retrieved when a
category exceeds 128 entries. Context ranks retrieved candidates; it does not scan
unretrieved files. Common-name substring pools can also omit better word-boundary
matches beyond the cap. Sparse searches cost more ES calls; history cannot rescue
an item absent from the candidate pool. Native filesystem and association behavior
is unchanged. Interactive desktop regression, live file-opening history, and a
new installer were not verified or produced for this task.
