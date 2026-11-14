import { highlightParts, parentFolder, type FileResult } from "../fileSearch";

function Highlight({ text, query }: { text: string; query: string }) {
  return <>{highlightParts(text, query).map((part, index) => part.match
    ? <mark key={index}>{part.text}</mark> : <span key={index}>{part.text}</span>)}</>;
}

export function FileResultContent({ file, query }: { file: FileResult; query: string }) {
  const parent = parentFolder(file.full_path);
  return <>
    <span className="command-icon file-icon" aria-hidden="true">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
        {file.is_directory ? <path d="M3 7V5a1 1 0 0 1 1-1h5l2 3h9a1 1 0 0 1 1 1v11H3V7Z" />
          : <><path d="M5 3h9l5 5v13H5V3Z" /><path d="M14 3v6h5M8 13h8M8 17h5" /></>}
      </svg>
    </span>
    <span className="command-copy">
      <span className="command-title" title={file.filename}><Highlight text={file.filename} query={query} /></span>
      <span className="command-description" title={file.full_path}><Highlight text={parent} query={query} /></span>
    </span>
    <span className="command-badge">{file.is_directory ? "Folder" : "File"}</span>
  </>;
}
