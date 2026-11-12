//! Private persistent downloads. A completed tree is assembled only from verified bytes.
use crate::{
    selection::{Artifact, Plan},
    sha256, Result, Source, ARTIFACT_LIMIT, ORIGIN,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

#[derive(Debug, PartialEq, Eq)]
/// Outcome of a [`crate::Source::fetch_from`] request.
///
/// This describes transfer progress, not payload authenticity; the caller checks
/// its expected complete size/hash before publishing a snapshot.
pub enum Transfer {
    /// The requested remainder was written; verify the complete file before reuse.
    Complete,
    /// Range continuation is unavailable or invalid; no bytes were written by this call.
    Restart,
}

pub(crate) fn http_range(
    agent: &ureq::Agent,
    url: &str,
    out: &mut dyn Write,
    offset: u64,
    limit: u64,
) -> Result<Transfer> {
    crate::events::check()?;
    let mut request = agent.get(url).set("Accept-Encoding", "identity");
    if offset > 0 {
        request = request.set("Range", &format!("bytes={offset}-"));
    }
    let response = match request.call() {
        Ok(r) => r,
        Err(ureq::Error::Status(416, _)) if offset > 0 => return Ok(Transfer::Restart),
        Err(e) => return Err(crate::network::transport_error(e)),
    };
    if offset > 0 && response.status() == 200 {
        return Ok(Transfer::Restart);
    }
    if response
        .header("Content-Encoding")
        .is_some_and(|v| v != "identity")
    {
        return Err("encoded response cannot be resumed safely".into());
    }
    let expected = if offset > 0 {
        if response.status() != 206 {
            return Err("resume expected HTTP 206".into());
        }
        let header = response
            .header("Content-Range")
            .ok_or("missing Content-Range")?;
        let (span, total) = header
            .strip_prefix("bytes ")
            .ok_or("invalid Content-Range unit")?
            .split_once('/')
            .ok_or("invalid Content-Range")?;
        let (start, end) = span.split_once('-').ok_or("invalid Content-Range span")?;
        let (start, end, total) = (number(start)?, number(end)?, number(total)?);
        if start != offset
            || end < start
            || end.checked_add(1) != Some(total)
            || total > ARTIFACT_LIMIT
            || total - offset > limit
        {
            return Err(crate::Error::new(
                crate::ErrorKind::Budget,
                "resume range exceeds expected offset or byte budget",
            )
            .into());
        }
        Some(total - offset)
    } else {
        if response.status() != 200 {
            return Err("download expected HTTP 200".into());
        }
        None
    };
    let length = response.header("Content-Length").map(number).transpose()?;
    if length.is_some_and(|n| n > limit) || expected.zip(length).is_some_and(|(a, b)| a != b) {
        return Err(crate::Error::new(
            crate::ErrorKind::Budget,
            "response length exceeds budget or disagrees with range",
        )
        .into());
    }
    let total = expected.or(length).and_then(|n| n.checked_add(offset));
    crate::events::artifact(
        url.strip_prefix(ORIGIN).unwrap_or("HTTP artifact"),
        if offset > 0 { "resume" } else { "download" },
        offset,
        total,
    )?;
    let mut reader = crate::network::NetworkReader(response.into_reader());
    let mut bytes = 0;
    let mut buffer = [0; 64 * 1024];
    loop {
        crate::events::check()?;
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        bytes += n as u64;
        if bytes > limit || expected.is_some_and(|max| bytes > max) {
            return Err(crate::Error::new(
                crate::ErrorKind::Budget,
                "response exceeds download budget",
            )
            .into());
        }
        out.write_all(&buffer[..n])?;
    }
    if expected.or(length).is_some_and(|n| n != bytes) {
        return Err("response ended before expected length".into());
    }
    Ok(Transfer::Complete)
}
fn number(s: &str) -> Result<u64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err("invalid HTTP byte count".into());
    }
    Ok(s.parse()?)
}
fn regular(path: &Path) -> Result<Option<u64>> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_file() && !m.file_type().is_symlink() => Ok(Some(m.len())),
        Ok(_) => Err("download state must contain regular files, not links/devices".into()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
fn directory(path: &Path) -> Result<()> {
    let m = fs::symlink_metadata(path)?;
    if !m.is_dir() || m.file_type().is_symlink() {
        return Err("download directory must not be a link".into());
    }
    Ok(())
}
pub(crate) fn hash_file(path: &Path, limit: u64) -> Result<(u64, String)> {
    if regular(path)?.ok_or("missing download file")? > limit {
        return Err(crate::Error::new(
            crate::ErrorKind::Budget,
            "stored download exceeds byte budget",
        )
        .into());
    }
    let mut f = File::open(path)?;
    let label = path.to_string_lossy();
    let total = fs::metadata(path)?.len();
    crate::events::hashing(&label, 0, Some(total))?;
    let mut count = 0;
    let mut hash = Sha256::new();
    let mut buf = [0; 64 * 1024];
    loop {
        crate::events::check()?;
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        count += n as u64;
        if count > limit {
            return Err(crate::Error::new(
                crate::ErrorKind::Budget,
                "stored download grew beyond budget",
            )
            .into());
        }
        hash.update(&buf[..n]);
        crate::events::hashing(&label, count, Some(total))?;
    }
    Ok((count, hex::encode(hash.finalize())))
}
/// Enumerate only expected native paths. Never reinterpret Unix backslashes as separators.
pub(crate) fn validate_tree(root: &Path, files: &BTreeSet<PathBuf>) -> Result<()> {
    directory(root)?;
    let mut dirs = BTreeSet::new();
    for file in files {
        let mut p = file.parent();
        while let Some(parent) = p {
            if parent.as_os_str().is_empty() {
                break;
            }
            dirs.insert(parent.to_path_buf());
            p = parent.parent();
        }
    }
    let mut stack = vec![root.to_path_buf()];
    let mut visited = 0;
    while let Some(dir) = stack.pop() {
        crate::events::check()?;
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            visited += 1;
            if visited > files.len() + dirs.len() {
                return Err("unexpected download entries".into());
            }
            let path = entry.path();
            let rel = path.strip_prefix(root)?;
            let m = fs::symlink_metadata(&path)?;
            if m.file_type().is_symlink() {
                return Err("download trees must not contain links".into());
            }
            if m.is_dir() && dirs.contains(rel) {
                stack.push(path);
            } else if !m.is_file() || !files.contains(rel) || m.len() > ARTIFACT_LIMIT {
                return Err("unexpected or oversized download entry".into());
            }
        }
    }
    Ok(())
}
#[derive(Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct State {
    schema: u32,
    plan_sha256: String,
}
pub(crate) struct Session {
    pub root: PathBuf,
    files: PathBuf,
    allowed: BTreeSet<PathBuf>,
    _lock: File,
}
impl Session {
    pub fn open(output: &Path, plan: &Plan, resume: bool) -> Result<Self> {
        if !resume {
            fs::create_dir(output)?;
        }
        directory(output)?;
        for e in fs::read_dir(output)? {
            let e = e?;
            if ![".fetch.lock", ".download", "mirror"]
                .iter()
                .any(|n| e.file_name() == *n)
            {
                return Err("output contains unrelated files".into());
            }
        }
        let lock = output.join(".fetch.lock");
        if regular(&lock)?.is_some_and(|n| n != 0) {
            return Err(
                crate::Error::new(crate::ErrorKind::Locked, "invalid download lock file").into(),
            );
        }
        let lock = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock)?;
        lock.try_lock().map_err(|_| {
            crate::Error::new(
                crate::ErrorKind::Locked,
                "another fetch owns this output, or filesystem locking is unavailable",
            )
        })?;
        let root = output.join(".download");
        let state = State {
            schema: 1,
            plan_sha256: sha256(toml::to_string_pretty(plan)?.as_bytes()),
        };
        if resume {
            directory(&root)?;
            let file = root.join("state.toml");
            if regular(&file)?.ok_or_else(|| {
                crate::Error::new(crate::ErrorKind::ResumeMismatch, "missing resume identity")
            })? > 4096
            {
                return Err(crate::Error::new(
                    crate::ErrorKind::ResumeMismatch,
                    "oversized resume identity",
                )
                .into());
            }
            let saved: State = toml::from_str(&fs::read_to_string(file)?)?;
            if saved != state {
                return Err(crate::Error::new(
                    crate::ErrorKind::ResumeMismatch,
                    "output belongs to a different frozen plan",
                )
                .into());
            }
        } else {
            fs::create_dir(&root)?;
            let mut f = File::create_new(root.join("state.toml"))?;
            f.write_all(toml::to_string(&state)?.as_bytes())?;
            f.sync_all()?;
            fs::create_dir(root.join("files"))?;
        }
        let files = root.join("files");
        directory(&files)?;
        for entry in fs::read_dir(&root)? {
            if !["state.toml", "files", "tree"]
                .iter()
                .any(|n| entry.as_ref().is_ok_and(|e| e.file_name() == *n))
            {
                return Err("unexpected download workspace entry".into());
            }
        }
        let allowed = plan
            .artifacts
            .iter()
            .flat_map(|a| {
                [
                    PathBuf::from(format!("{}.data", a.sha256)),
                    PathBuf::from(format!("{}.part", a.sha256)),
                ]
            })
            .collect();
        validate_tree(&files, &allowed)?;
        Ok(Self {
            root,
            files,
            allowed,
            _lock: lock,
        })
    }
    pub fn stage(&self, expected: &BTreeSet<PathBuf>) -> Result<PathBuf> {
        let tree = self.root.join("tree");
        if fs::symlink_metadata(&tree).is_ok() {
            validate_tree(&tree, expected)?;
            fs::remove_dir_all(&tree)?;
        }
        fs::create_dir(&tree)?;
        Ok(tree)
    }
    pub fn cleanup(&self) -> Result<()> {
        validate_tree(&self.files, &self.allowed)?;
        for entry in fs::read_dir(&self.files)? {
            fs::remove_file(entry?.path())?;
        }
        Ok(())
    }
    pub fn retrieve(
        &self,
        a: &Artifact,
        source: &impl Source,
        limit: u64,
    ) -> Result<(PathBuf, u64)> {
        let ready = self.files.join(format!("{}.data", a.sha256));
        let part = self.files.join(format!("{}.part", a.sha256));
        if regular(&ready)?.is_some() {
            if let Ok((n, h)) = hash_file(&ready, limit) {
                if h == a.sha256 {
                    eprintln_status("Reuse", a, n);
                    crate::events::artifact(&a.path, "reuse", n, Some(n))?;
                    return Ok((ready, n));
                }
            }
            fs::remove_file(&ready)?;
        }
        let mut offset = regular(&part)?.unwrap_or(0);
        if offset > limit {
            fs::remove_file(&part)?;
            offset = 0;
        }
        if offset > 0 {
            let (n, h) = hash_file(&part, limit)?;
            if h == a.sha256 {
                crate::events::check()?;
                fs::rename(&part, &ready)?;
                return Ok((ready, n));
            }
        }
        let mut file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&part)?;
        file.seek(SeekFrom::End(0))?;
        eprintln_status(if offset > 0 { "Resume" } else { "Download" }, a, offset);
        crate::events::artifact(
            &a.path,
            if offset > 0 { "resume" } else { "download" },
            offset,
            None,
        )?;
        let transfer = source.fetch_from(
            &format!("{ORIGIN}{}", a.path),
            &mut crate::events::ProgressWriter {
                inner: &mut file,
                path: a.path.clone(),
                count: offset,
            },
            offset,
            limit - offset,
        );
        file.sync_all()?;
        let transfer = transfer?;
        if transfer == Transfer::Restart {
            file.set_len(0)?;
            file.seek(SeekFrom::Start(0))?;
            eprintln_status("Restart", a, 0);
            crate::events::artifact(&a.path, "restart", 0, None)?;
            let transfer = source.fetch_from(
                &format!("{ORIGIN}{}", a.path),
                &mut crate::events::ProgressWriter {
                    inner: &mut file,
                    path: a.path.clone(),
                    count: 0,
                },
                0,
                limit,
            );
            file.sync_all()?;
            if transfer? != Transfer::Complete {
                return Err("source cannot restart download".into());
            }
        }
        drop(file);
        let (n, h) = hash_file(&part, limit)?;
        if h != a.sha256 {
            fs::remove_file(&part)?;
            return Err(
                format!("download checksum mismatch; discarded partial: {}", a.path).into(),
            );
        }
        crate::events::check()?;
        fs::rename(&part, &ready)?;
        Ok((ready, n))
    }
}
fn eprintln_status(action: &str, a: &Artifact, n: u64) {
    let _ = writeln!(io::stderr().lock(), "{action}: {} ({n} bytes)", a.path);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    fn response(
        status: &str,
        headers: &str,
        body: &str,
        offset: u64,
        limit: u64,
    ) -> (Result<Transfer>, Vec<u8>, String) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let bytes = format!("HTTP/1.1 {status}\r\nConnection: close\r\n{headers}\r\n{body}");
        let thread = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut data = Vec::new();
            let mut byte = [0];
            while socket.read(&mut byte).unwrap_or(0) > 0 {
                data.push(byte[0]);
                if data.ends_with(b"\r\n\r\n") {
                    break;
                }
            }
            let _ = socket.write_all(bytes.as_bytes());
            String::from_utf8(data).unwrap()
        });
        let agent = ureq::AgentBuilder::new()
            .redirects(0)
            .timeout(std::time::Duration::from_secs(5))
            .build();
        let mut out = Vec::new();
        let result = http_range(
            &agent,
            &format!("http://{address}/file"),
            &mut out,
            offset,
            limit,
        );
        (result, out, thread.join().unwrap())
    }
    #[test]
    fn valid_range_and_full_responses_preserve_wire_bytes() {
        let (r, b, h) = response(
            "206 Partial Content",
            "Content-Range: bytes 2-5/6\r\nContent-Length: 4\r\n",
            "cdef",
            2,
            4,
        );
        assert_eq!(r.unwrap(), Transfer::Complete);
        assert_eq!(b, b"cdef");
        assert!(h.to_lowercase().contains("range: bytes=2-"));
        assert!(h.to_lowercase().contains("accept-encoding: identity"));
        let (r, b, _) = response("200 OK", "Content-Length: 6\r\n", "abcdef", 0, 6);
        assert_eq!(r.unwrap(), Transfer::Complete);
        assert_eq!(b, b"abcdef");
    }
    #[test]
    fn ignored_and_unsatisfiable_ranges_request_restart_without_appending() {
        for status in ["200 OK", "416 Range Not Satisfiable"] {
            let (r, b, _) = response(status, "Content-Length: 3\r\n", "bad", 2, 4);
            assert_eq!(r.unwrap(), Transfer::Restart);
            assert!(b.is_empty());
        }
    }
    #[test]
    fn malformed_ranges_encoding_redirects_and_oversize_headers_refuse() {
        for (status, headers, offset, limit) in [
            ("206 Partial Content", "Content-Length: 4\r\n", 2, 4),
            (
                "206 Partial Content",
                "Content-Range: bytes 1-5/6\r\n",
                2,
                4,
            ),
            (
                "206 Partial Content",
                "Content-Range: bytes 2-4/6\r\n",
                2,
                4,
            ),
            (
                "206 Partial Content",
                "Content-Range: bytes 2-5/*\r\n",
                2,
                4,
            ),
            (
                "206 Partial Content",
                "Content-Range: bytes 2-5/6\r\nContent-Length: 3\r\n",
                2,
                4,
            ),
            (
                "206 Partial Content",
                "Content-Range: bytes 2-5/6\r\n",
                2,
                3,
            ),
            ("200 OK", "Content-Length: 6\r\n", 0, 5),
            (
                "200 OK",
                "Content-Encoding: br\r\nContent-Length: 4\r\n",
                0,
                4,
            ),
            (
                "302 Found",
                "Location: http://127.0.0.1:1/elsewhere\r\nContent-Length: 0\r\n",
                2,
                4,
            ),
            (
                "206 Partial Content",
                "Content-Range: bytes 2-18446744073709551615/0\r\n",
                2,
                4,
            ),
        ] {
            let (r, b, _) = response(status, headers, "cdef", offset, limit);
            assert!(r.is_err(), "{headers}");
            assert!(b.is_empty(), "{headers}");
        }
    }
    #[test]
    fn truncated_and_unbounded_bodies_never_report_success() {
        let (r, b, _) = response(
            "206 Partial Content",
            "Content-Range: bytes 2-5/6\r\nContent-Length: 4\r\n",
            "cd",
            2,
            4,
        );
        assert!(r.is_err());
        assert!(b.len() <= 2);
        let (r, b, _) = response("200 OK", "", "abcdef", 0, 3);
        assert!(r.is_err());
        assert!(b.len() <= 3);
    }
}
