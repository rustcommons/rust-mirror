//! Fixed-origin crate metadata/archive transport and bounded cancellation-aware retries.
use super::catalog::Transport;
use super::{budget, index_path, invalid};
use crate::{events, network, Error, ErrorKind, NetworkOptions, Result, Transfer};
use std::{
    io::{Read, Write},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// Separated registry operations: planning uses only index and optional HEAD, never archive GET.
pub trait CrateSource: Send + Sync {
    /// Fetch bounded original sparse metadata for one canonical package.
    fn index(&self, name: &str, limit: usize) -> Result<Vec<u8>>;
    /// Optional archive-length estimate. Unknown is not zero.
    fn head(&self, _name: &str, _version: &str) -> Result<Option<u64>> {
        Ok(None)
    }
    /// Transfer an archive/remainder within the supplied remaining byte budget.
    fn archive(
        &self,
        name: &str,
        version: &str,
        out: &mut dyn Write,
        offset: u64,
        limit: u64,
    ) -> Result<Transfer>;
    /// Configured maximum attempts, including the first.
    fn attempts(&self) -> usize {
        1
    }
    /// Return a safe bounded wait for a transient failure; permanent failures return None.
    fn retry(
        &self,
        _error: &(dyn std::error::Error + 'static),
        _attempt: usize,
        _path: &str,
    ) -> Result<bool> {
        Ok(false)
    }
}
/// Official crates.io sparse/CDN source with explicit proxy/CA options and public contact.
pub struct RegistrySource {
    index_agent: ureq::Agent,
    archive_agent: ureq::Agent,
    user_agent: String,
    policy: Transport,
    #[cfg(test)]
    fixture_origin: Option<String>,
}
impl RegistrySource {
    /// Validate public scheduling/contact options and construct independent origin-bound agents.
    pub fn new(network: &NetworkOptions, policy: Transport) -> Result<Self> {
        if !(1..=16).contains(&policy.concurrency)
            || !(1..=8).contains(&policy.attempts)
            || !(1..=300).contains(&policy.max_wait_seconds)
            || policy.contact.len() > 512
            || !policy.contact.is_ascii()
            || policy.contact.chars().any(char::is_control)
        {
            return Err(invalid("invalid crate network scheduling/contact"));
        }
        Ok(Self {
            index_agent: network::agent(network, "index.crates.io", 443)?,
            archive_agent: network::agent(network, "static.crates.io", 443)?,
            user_agent: format!(
                "rust-mirror/{} (+https://github.com/rustcommons/rust-mirror; {})",
                env!("CARGO_PKG_VERSION"),
                policy.contact
            ),
            policy,
            #[cfg(test)]
            fixture_origin: None,
        })
    }
    fn url(&self, index: bool, path: &str) -> String {
        #[cfg(test)]
        if let Some(origin) = &self.fixture_origin {
            return format!("{origin}/{path}");
        }
        format!(
            "https://{}/{path}",
            if index {
                "index.crates.io"
            } else {
                "static.crates.io"
            }
        )
    }
    fn call(&self, request: ureq::Request) -> Result<ureq::Response> {
        events::check()?;
        match request
            .set("User-Agent", &self.user_agent)
            .set("Accept-Encoding", "identity")
            .call()
        {
            Ok(response) => Ok(response),
            Err(ureq::Error::Status(status, response)) if status == 429 || status == 503 => {
                let mut error = Error::new(
                    ErrorKind::Network,
                    if status == 429 {
                        "upstream rate limit"
                    } else {
                        "upstream temporarily unavailable"
                    },
                );
                let mut report = toml::map::Map::new();
                report.insert(
                    "retry_kind".into(),
                    toml::Value::String(
                        if status == 429 {
                            "rate_limited"
                        } else {
                            "unavailable"
                        }
                        .into(),
                    ),
                );
                if let Some(header) = response.header("Retry-After") {
                    let seconds = retry_after(
                        header,
                        SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
                    )
                    .ok_or_else(|| {
                        invalid("invalid upstream Retry-After; refusing automatic retry")
                    })?;
                    report.insert(
                        "wait_seconds".into(),
                        toml::Value::Integer(seconds.min(i64::MAX as u64) as i64),
                    );
                }
                error.report = Some(toml::Value::Table(report));
                Err(error.into())
            }
            Err(ureq::Error::Status(status, _)) => {
                let mut error = Error::new(
                    if matches!(status, 401 | 403 | 407) {
                        ErrorKind::Auth
                    } else {
                        ErrorKind::Network
                    },
                    "upstream refused the request",
                );
                let mut report = toml::map::Map::new();
                report.insert("status".into(), toml::Value::Integer(status as i64));
                report.insert("retryable".into(), toml::Value::Boolean(false));
                error.report = Some(toml::Value::Table(report));
                Err(error.into())
            }
            Err(error) => Err(network::transport_error(error)),
        }
    }
    fn metadata(&self, name: &str, limit: usize) -> Result<Vec<u8>> {
        let path = index_path(name)?;
        let response = self.call(self.index_agent.get(&self.url(true, &path)))?;
        if response.status() != 200 {
            return Err(invalid("sparse metadata did not return HTTP 200"));
        }
        let mut reader = network::NetworkReader(response.into_reader());
        let mut data = Vec::new();
        let mut buffer = [0; 16384];
        loop {
            events::check()?;
            let count = reader.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            if data.len().saturating_add(count) > limit {
                return Err(budget("sparse metadata input exceeds bound"));
            }
            data.extend_from_slice(&buffer[..count]);
        }
        Ok(data)
    }
}
/// Strict archive path derived from a validated registry identity, never from a saved arbitrary URL.
pub fn archive_path(name: &str, version: &str) -> Result<String> {
    index_path(name)?;
    semver::Version::parse(version).map_err(|_| invalid("invalid crate version"))?;
    if version.len() > 128
        || !version
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-+".contains(&b))
    {
        return Err(invalid("invalid crate archive version path"));
    }
    Ok(format!("cargo/crates/{name}/{name}-{version}.crate"))
}
impl CrateSource for RegistrySource {
    fn index(&self, name: &str, limit: usize) -> Result<Vec<u8>> {
        let path = format!("cargo/index/{}", index_path(name)?);
        for attempt in 1..=self.policy.attempts {
            events::artifact(&path, "download", 0, None)?;
            match self.metadata(name, limit) {
                Ok(data) => {
                    events::complete(&path, "download", Some(data.len() as u64), false)?;
                    return Ok(data);
                }
                Err(error) => {
                    if !self.retry(error.as_ref(), attempt, &path)? {
                        return Err(error);
                    }
                }
            }
        }
        unreachable!()
    }
    fn head(&self, name: &str, version: &str) -> Result<Option<u64>> {
        let path = archive_path(name, version)?;
        let upstream = path.strip_prefix("cargo/").unwrap();
        for attempt in 1..=self.policy.attempts {
            events::artifact(&path, "head", 0, None)?;
            match self.call(self.archive_agent.head(&self.url(false, upstream))) {
                Ok(response) => {
                    if response.status() != 200 {
                        return Err(invalid("archive HEAD did not return HTTP 200"));
                    }
                    if response
                        .header("Content-Encoding")
                        .is_some_and(|v| v != "identity")
                    {
                        return Err(crate::Error::new(
                            crate::ErrorKind::Integrity,
                            "unexpected encoded HEAD estimate",
                        )
                        .into());
                    }
                    let lengths = response.all("Content-Length");
                    let mut length = None;
                    for value in lengths {
                        let n = number(value)?;
                        if n > i64::MAX as u64 {
                            return Err(budget("HEAD length exceeds event integer bounds"));
                        }
                        if length.is_some_and(|old| old != n) {
                            return Err(Error::new(
                                ErrorKind::Integrity,
                                "conflicting archive HEAD lengths",
                            )
                            .into());
                        }
                        length = Some(n);
                    }
                    events::complete(&path, "head", length, false)?;
                    return Ok(length);
                }
                Err(error) => {
                    if error
                        .downcast_ref::<Error>()
                        .and_then(|e| e.report.as_ref())
                        .and_then(|v| v.get("status"))
                        .and_then(toml::Value::as_integer)
                        .is_some_and(|s| s == 405 || s == 501)
                    {
                        events::complete(&path, "head", None, false)?;
                        return Ok(None);
                    }
                    if !self.retry(error.as_ref(), attempt, &path)? {
                        return Err(error);
                    }
                }
            }
        }
        unreachable!()
    }
    fn archive(
        &self,
        name: &str,
        version: &str,
        out: &mut dyn Write,
        offset: u64,
        limit: u64,
    ) -> Result<Transfer> {
        let path = archive_path(name, version)?;
        let upstream = path.strip_prefix("cargo/").unwrap();
        let mut request = self.archive_agent.get(&self.url(false, upstream));
        if offset > 0 {
            request = request.set("Range", &format!("bytes={offset}-"));
        }
        let response = match self.call(request) {
            Err(error)
                if offset > 0
                    && error
                        .downcast_ref::<Error>()
                        .and_then(|e| e.report.as_ref())
                        .and_then(|v| v.get("status"))
                        .and_then(toml::Value::as_integer)
                        == Some(416) =>
            {
                return Ok(Transfer::Restart)
            }
            result => result?,
        };
        if offset > 0 && response.status() == 200 {
            return Ok(Transfer::Restart);
        }
        if response
            .header("Content-Encoding")
            .is_some_and(|v| v != "identity")
        {
            return Err(Error::new(
                ErrorKind::Integrity,
                "encoded crate response cannot be resumed",
            )
            .into());
        }
        let expected = if offset > 0 {
            if response.status() != 206 {
                return Err(invalid("crate range expected HTTP 206"));
            }
            let (span, total) = response
                .header("Content-Range")
                .and_then(|s| s.strip_prefix("bytes "))
                .and_then(|s| s.split_once('/'))
                .ok_or_else(|| invalid("missing or invalid crate Content-Range"))?;
            let (start, end) = span
                .split_once('-')
                .ok_or_else(|| invalid("invalid crate byte range"))?;
            let (start, end, total) = (number(start)?, number(end)?, number(total)?);
            if start != offset
                || end < start
                || end.checked_add(1) != Some(total)
                || total - offset > limit
            {
                return Err(budget(
                    "crate range exceeds offset or remaining byte budget",
                ));
            }
            Some(total - offset)
        } else {
            if response.status() != 200 {
                return Err(invalid("crate download expected HTTP 200"));
            }
            None
        };
        let mut length = None;
        for value in response.all("Content-Length") {
            let n = number(value)?;
            if length.is_some_and(|old| old != n) {
                return Err(invalid("conflicting archive lengths"));
            }
            length = Some(n);
        }
        if length.is_some_and(|n| n > limit) || length.zip(expected).is_some_and(|(a, b)| a != b) {
            return Err(budget("crate response length exceeds budget or range"));
        }
        events::artifact(
            &path,
            if offset > 0 { "resume" } else { "download" },
            offset,
            length.or(expected).and_then(|n| n.checked_add(offset)),
        )?;
        let mut reader = network::NetworkReader(response.into_reader());
        let mut count = 0;
        let mut buffer = [0; 65536];
        loop {
            events::check()?;
            let n = reader.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            count += n as u64;
            if count > limit || expected.is_some_and(|n| count > n) {
                return Err(budget("crate stream exceeds bound"));
            }
            out.write_all(&buffer[..n])?;
            events::bytes(&path, offset + count)?;
        }
        if expected.or(length).is_some_and(|n| n != count) {
            return Err(Error::new(
                ErrorKind::Network,
                "crate stream ended before advertised length",
            )
            .into());
        }
        events::complete(
            &path,
            if offset > 0 { "resume" } else { "download" },
            Some(offset + count),
            false,
        )?;
        Ok(Transfer::Complete)
    }
    fn attempts(&self) -> usize {
        self.policy.attempts
    }
    fn retry(
        &self,
        error: &(dyn std::error::Error + 'static),
        attempt: usize,
        path: &str,
    ) -> Result<bool> {
        if attempt >= self.policy.attempts || crate::error_kind(error) != ErrorKind::Network {
            return Ok(false);
        }
        let report = error
            .downcast_ref::<Error>()
            .and_then(|e| e.report.as_ref());
        if report
            .and_then(|v| v.get("retryable"))
            .and_then(toml::Value::as_bool)
            == Some(false)
        {
            return Ok(false);
        }
        let kind = match report
            .and_then(|v| v.get("retry_kind"))
            .and_then(toml::Value::as_str)
        {
            Some("rate_limited") => events::RetryKind::RateLimited,
            Some("unavailable") => events::RetryKind::Unavailable,
            _ => events::RetryKind::Transport,
        };
        let wait = report
            .and_then(|v| v.get("wait_seconds"))
            .and_then(toml::Value::as_integer)
            .map(|n| n as u64)
            .unwrap_or((1u64 << attempt.min(8)).min(self.policy.max_wait_seconds));
        if wait > self.policy.max_wait_seconds {
            return Ok(false);
        }
        if let Some(control) = events::current() {
            control.emit(events::EventKind::Retry {
                path: path.into(),
                attempt: (attempt + 1) as u64,
                wait_seconds: wait,
                kind,
                reason: match kind {
                    events::RetryKind::RateLimited => "server requested reduced request rate",
                    events::RetryKind::Unavailable => "service temporarily unavailable",
                    events::RetryKind::Transport => "transient transport failure",
                }
                .into(),
            })?;
        }
        let until = Instant::now() + Duration::from_secs(wait);
        while Instant::now() < until {
            events::check()?;
            std::thread::park_timeout(
                until
                    .saturating_duration_since(Instant::now())
                    .min(Duration::from_millis(100)),
            );
        }
        events::check()?;
        Ok(true)
    }
}
fn number(text: &str) -> Result<u64> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid("invalid HTTP byte count"));
    }
    text.parse().map_err(|_| budget("HTTP byte count overflow"))
}
fn retry_after(text: &str, now: u64) -> Option<u64> {
    if !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()) {
        return text.parse().ok();
    }
    let parts: Vec<_> = text.split_ascii_whitespace().collect();
    if parts.len() != 6 || parts[5] != "GMT" || !parts[0].ends_with(',') {
        return None;
    }
    let day: i64 = parts[1].parse().ok()?;
    let month: i64 = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ]
    .iter()
    .position(|m| *m == parts[2])? as i64
        + 1;
    let year: i64 = parts[3].parse().ok()?;
    if !(1970..=9999).contains(&year) || !(1..=31).contains(&day) {
        return None;
    }
    let mut time = parts[4].split(':');
    let hour: u64 = time.next()?.parse().ok()?;
    let minute: u64 = time.next()?.parse().ok()?;
    let second: u64 = time.next()?.parse().ok()?;
    if time.next().is_some() || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if day > days[(month - 1) as usize] {
        return None;
    }
    let y = year - i64::from(month <= 2);
    let era = y / 400;
    let yoe = y - era * 400;
    let m = month + if month > 2 { -3 } else { 9 };
    let ordinal = (153 * m + 2) / 5 + day - 1;
    let days = era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + ordinal - 719468;
    let seconds = u64::try_from(days)
        .ok()?
        .checked_mul(86400)?
        .checked_add(hour * 3600 + minute * 60 + second)?;
    Some(seconds.saturating_sub(now))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retry_delay_formats_and_caps() {
        assert_eq!(retry_after("12", 0), Some(12));
        assert_eq!(retry_after("Thu, 01 Jan 1970 00:00:30 GMT", 0), Some(30));
        assert_eq!(
            retry_after("Wed, 21 Oct 2015 07:28:00 GMT", 1445412480),
            Some(0)
        );
        assert_eq!(retry_after("Wed, 31 Feb 2024 07:28:00 GMT", 0), None);
        assert_eq!(retry_after("bad", 0), None);
    }
}

#[cfg(test)]
mod http_tests {
    use super::*;
    use std::{net::TcpListener, thread};
    fn fixture(replies: Vec<String>) -> (RegistrySource, thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let mut requests = Vec::new();
            for reply in replies {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut data = Vec::new();
                let mut byte = [0];
                while !data.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut byte).unwrap();
                    data.extend_from_slice(&byte);
                    assert!(data.len() < 16384);
                }
                requests.push(String::from_utf8(data).unwrap());
                stream.write_all(reply.as_bytes()).unwrap();
            }
            requests
        });
        let mut source = RegistrySource::new(
            &NetworkOptions::default(),
            Transport {
                max_wait_seconds: 1,
                ..Transport::default()
            },
        )
        .unwrap();
        source.fixture_origin = Some(format!("http://{address}"));
        (source, handle)
    }
    fn reply(status: u16, headers: &str, body: &str) -> String {
        format!("HTTP/1.1 {status} Fixture\r\nConnection: close\r\nContent-Length: {}\r\n{headers}\r\n{body}",body.len())
    }
    #[test]
    fn rate_limit_retries_but_permanent_status_does_not() {
        let (source, server) = fixture(vec![
            reply(429, "Retry-After: 0\r\n", ""),
            reply(200, "", "{}\n"),
        ]);
        assert_eq!(source.index("x", 1024).unwrap(), b"{}\n");
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests[0]
            .to_ascii_lowercase()
            .contains("user-agent: rust-mirror/"));
        let (source, server) = fixture(vec![reply(404, "", "")]);
        let error = source.index("x", 1024).unwrap_err();
        assert_eq!(
            error
                .downcast_ref::<Error>()
                .unwrap()
                .report
                .as_ref()
                .unwrap()["status"]
                .as_integer(),
            Some(404)
        );
        assert_eq!(server.join().unwrap().len(), 1);
    }
    #[test]
    fn long_server_wait_is_not_shortened_and_unsupported_head_stays_unknown() {
        let (source, server) = fixture(vec![reply(503, "Retry-After: 300\r\n", "")]);
        let begin = Instant::now();
        assert!(source.index("x", 1024).is_err());
        assert!(begin.elapsed() < Duration::from_secs(3));
        server.join().unwrap();
        let (source, server) = fixture(vec![reply(405, "", "")]);
        assert_eq!(source.head("x", "1.0.0").unwrap(), None);
        assert!(server.join().unwrap()[0].starts_with("HEAD "));
    }
    #[test]
    fn validated_ranges_and_metadata_budgets() {
        let (source, server) = fixture(vec![reply(206, "Content-Range: bytes 3-5/6\r\n", "def")]);
        let mut output = Vec::new();
        assert_eq!(
            source.archive("x", "1.0.0", &mut output, 3, 100).unwrap(),
            Transfer::Complete
        );
        assert_eq!(output, b"def");
        assert!(server.join().unwrap()[0]
            .to_ascii_lowercase()
            .contains("range: bytes=3-"));
        let (source, server) = fixture(vec![reply(206, "Content-Range: bytes 2-4/6\r\n", "def")]);
        let mut output = Vec::new();
        assert!(source.archive("x", "1.0.0", &mut output, 3, 100).is_err());
        assert!(output.is_empty());
        server.join().unwrap();
        let (source, server) = fixture(vec![reply(416, "", "")]);
        assert_eq!(
            source
                .archive("x", "1.0.0", &mut Vec::new(), 3, 100)
                .unwrap(),
            Transfer::Restart
        );
        server.join().unwrap();
        let (source, server) = fixture(vec![reply(200, "", "large")]);
        let error = source.index("x", 1).unwrap_err();
        assert_eq!(crate::error_kind(error.as_ref()), ErrorKind::Budget);
        server.join().unwrap();
    }
}
