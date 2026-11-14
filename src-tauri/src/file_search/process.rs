use super::{ErrorCode, Result, SearchError};
use std::{
    io::Read,
    path::Path,
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

pub(super) struct Response {
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
}
struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn read_bounded(reader: impl Read, max: usize) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take(max as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > max {
        return Err(std::io::Error::other("ES output exceeded its size limit"));
    }
    Ok(bytes)
}

pub(super) fn run(
    executable: &Path,
    args: &[String],
    timeout: Duration,
    max: usize,
) -> Result<Response> {
    let mut command = Command::new(executable);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let mut child = ChildGuard(command.spawn().map_err(|e| {
        SearchError::new(ErrorCode::ProcessFailed, format!("Could not start ES: {e}"))
    })?);
    let stdout = child.0.stdout.take().ok_or_else(|| {
        SearchError::new(ErrorCode::ProcessFailed, "ES output pipe was not created.")
    })?;
    let stderr = child.0.stderr.take().ok_or_else(|| {
        SearchError::new(ErrorCode::ProcessFailed, "ES error pipe was not created.")
    })?;
    // Drain both streams concurrently: neither a full stderr pipe nor oversized
    // stdout may block the child forever. Worker results have bounded allocations.
    let (send, receive) = mpsc::channel();
    let other = send.clone();
    std::thread::Builder::new()
        .name("nexus-es-stdout".into())
        .spawn(move || {
            let _ = send.send((true, read_bounded(stdout, max)));
        })
        .map_err(|e| SearchError::new(ErrorCode::ProcessFailed, e.to_string()))?;
    std::thread::Builder::new()
        .name("nexus-es-stderr".into())
        .spawn(move || {
            let _ = other.send((false, read_bounded(stderr, 16384)));
        })
        .map_err(|e| SearchError::new(ErrorCode::ProcessFailed, e.to_string()))?;
    let start = Instant::now();
    let mut output = None;
    let mut streams = 0;
    loop {
        while let Ok((is_stdout, result)) = receive.try_recv() {
            let bytes = result.map_err(|e| {
                SearchError::new(
                    ErrorCode::ProcessFailed,
                    format!("Could not read bounded ES output: {e}"),
                )
            })?;
            if is_stdout {
                output = Some(bytes);
            }
            streams += 1;
        }
        let status = child.0.try_wait().map_err(|e| {
            SearchError::new(ErrorCode::ProcessFailed, format!("Could not check ES: {e}"))
        })?;
        if let Some(status) = status {
            if streams == 2 {
                return Ok(Response {
                    code: status.code(),
                    stdout: output.unwrap_or_default(),
                });
            }
        }
        if start.elapsed() >= timeout {
            return Err(SearchError::new(
                ErrorCode::Timeout,
                "Everything did not finish searching within five seconds. Check that its database is ready and retry.",
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_reads_reject_excess_and_io_errors() {
        assert_eq!(read_bounded(&b"abc"[..], 3).unwrap(), b"abc");
        assert!(read_bounded(&b"abcd"[..], 3).is_err());
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("broken pipe"))
            }
        }
        assert!(read_bounded(Broken, 8).is_err());
    }
    #[test]
    #[ignore = "subprocess fixture, invoked by timeout_and_exit_are_bounded"]
    fn sleeping_child() {
        std::thread::sleep(Duration::from_secs(10));
    }
    #[test]
    fn timeout_and_exit_are_bounded() {
        let exe = std::env::current_exe().unwrap();
        let args = [
            "--exact",
            "file_search::process::tests::sleeping_child",
            "--ignored",
        ]
        .map(String::from);
        let start = Instant::now();
        assert_eq!(
            run(&exe, &args, Duration::from_millis(150), 4096)
                .err()
                .unwrap()
                .code,
            ErrorCode::Timeout
        );
        assert!(start.elapsed() < Duration::from_secs(3));
        let response = run(&exe, &["--list".into()], Duration::from_secs(3), 65536).unwrap();
        assert_eq!(response.code, Some(0));
        assert!(!response.stdout.is_empty());
        assert!(run(&exe, &["--list".into()], Duration::from_secs(3), 1).is_err());
        assert!(
            run(
                Path::new("Z:\\nexus-nonexistent-es.exe"),
                &[],
                Duration::from_secs(1),
                1024
            )
            .is_err()
        );
    }
}
