use std::{
    env, fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

use reqwest::Url;
use ripple_core::{
    Error, Result,
    document::Page,
    oauth::Credentials,
    oauth::{AuthAttempt, OAuthClient},
    provider::{EFFORT, MODEL, ModelClient, VisualIntent},
    quick::{PluginIntent, report},
    store::{AccountStore, atomic_json},
    workspace::Project,
};
use serde_json::{Value, json};
use tiny_http::{Header, Method, Request, Response, Server};

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
const INDEX: &str = include_str!("../../../examples/playground/index.html");

struct App {
    page_path: PathBuf,
    project_path: Option<PathBuf>,
    origin: String,
    host: String,
    accounts: AccountStore,
    oauth: Option<OAuthClient>,
    pending: Option<AuthAttempt>,
    models: ModelClient,
    catalog: Option<(String, Instant)>,
    running: Arc<AtomicBool>,
}

fn read_page(path: &Path) -> Result<Page> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(2_000_001)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 2_000_000 {
        return Err(Error::Invalid("Page document is too large."));
    }
    let page: Page = serde_json::from_slice(&bytes)?;
    page.validate()?;
    Ok(page)
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut port: u16 = 4310;
    let mut page_path = PathBuf::from("ripple.page.json");
    let mut project_path = None;
    let mut explicit_page = false;
    let mut check_only = false;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--port" => port = args.next().ok_or("Missing port")?.parse()?,
            "--page" => {
                explicit_page = true;
                page_path = PathBuf::from(args.next().ok_or("Missing page path")?);
            }
            "--project" => {
                project_path = Some(PathBuf::from(
                    args.next().ok_or("Missing project manifest")?,
                ))
            }
            "--check" => check_only = true,
            "--help" => {
                println!(
                    "ripple-dev [--port 4310] [--page ripple.page.json | --project ripple.project.json]"
                );
                println!(
                    "ripple-dev --project ripple.project.json --check  (validate a disposable copy, no OAuth)"
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown argument: {arg}").into()),
        }
    }
    if project_path.is_none() && !explicit_page && Path::new("ripple.project.json").try_exists()? {
        project_path = Some(PathBuf::from("ripple.project.json"));
    }
    if let Some(path) = &project_path {
        if explicit_page {
            return Err("Use --project or --page, not both".into());
        }
        let project = Project::open(path)?;
        page_path = if project.manifest.page.is_empty() {
            PathBuf::new()
        } else {
            project.page_path()
        };
    }
    if !page_path.as_os_str().is_empty() {
        read_page(&page_path)?;
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    if check_only {
        let project = Project::open(project_path.as_ref().ok_or("--check requires --project")?)?;
        let mut workspace = if project.manifest.page.is_empty() {
            let revision = project.revision()?;
            project.begin_sources(&revision)?
        } else {
            let page = project.page()?;
            project.begin(&page, &page.revision()?)?
        };
        for check in runtime.block_on(workspace.verify())? {
            println!("{}: passed", check.name);
        }
        return Ok(());
    }
    let home = if let Some(path) = env::var_os("RIPPLE_HOME") {
        PathBuf::from(path)
    } else if let Some(path) = env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(path).join("ripple")
    } else {
        PathBuf::from(
            env::var_os("HOME")
                .or_else(|| env::var_os("USERPROFILE"))
                .ok_or("No home directory")?,
        )
        .join(".config/ripple")
    };
    let accounts = AccountStore::open(home)?;
    let server = Server::http(("127.0.0.1", port))?;
    let address = server
        .server_addr()
        .to_ip()
        .ok_or("Missing server address")?;
    let origin = format!("http://{address}");
    let mut app = App {
        page_path,
        project_path,
        host: address.to_string(),
        origin: origin.clone(),
        accounts,
        oauth: None,
        pending: None,
        models: ModelClient::new()?,
        catalog: None,
        running: Arc::new(AtomicBool::new(false)),
    };
    println!("Ripple → {origin}\nGPT-6 Luna · low\nCtrl+C to stop.");
    for mut request in server.incoming_requests() {
        let cors = app.cors_origin(&request);
        let origin = cors.as_ref().ok().and_then(|origin| origin.clone());
        let result = if request.method() == &Method::Post && request.url() == "/api/plugin/change" {
            match cors {
                Err(error) => Err(error),
                Ok(_) => match runtime.block_on(app.start_plugin(&mut request, &runtime)) {
                    Ok(job) => {
                        std::thread::spawn(move || stream_job(request, job, origin));
                        continue;
                    }
                    Err(error) => Err(error),
                },
            }
        } else {
            match cors {
                Ok(_) => runtime.block_on(app.handle(&mut request)),
                Err(error) => Err(error),
            }
        };
        let (status, content_type, body, location) = match result {
            Ok(reply) => reply,
            Err(error) => (
                400,
                "application/json",
                serde_json::to_vec(&json!({"error":error.to_string()}))?,
                None,
            ),
        };
        let mut response = Response::from_data(body)
            .with_status_code(status)
            .with_header(header("Content-Type", content_type))
            .with_header(header("Cache-Control", "no-store"))
            .with_header(header("X-Content-Type-Options", "nosniff"));
        if let Some(origin) = origin {
            response.add_header(header("Access-Control-Allow-Origin", &origin));
            response.add_header(header("Vary", "Origin"));
        }
        if request.method() == &Method::Options && status == 200 {
            response.add_header(header("Access-Control-Allow-Methods", "POST"));
            response.add_header(header(
                "Access-Control-Allow-Headers",
                "Content-Type, X-Ripple-Request",
            ));
            response.add_header(header("Access-Control-Max-Age", "600"));
        }
        if let Some(location) = location {
            response.add_header(header("Location", &location));
        }
        if let Err(error) = request.respond(response) {
            eprintln!("Browser connection closed: {error}");
        }
    }
    Ok(())
}

type Reply = (u16, &'static str, Vec<u8>, Option<String>);
fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name, value).expect("Static HTTP header")
}
fn json_reply(value: Value) -> Result<Reply> {
    Ok((200, "application/json", serde_json::to_vec(&value)?, None))
}

impl App {
    async fn handle(&mut self, request: &mut Request) -> Result<Reply> {
        let host = request
            .headers()
            .iter()
            .find(|h| h.field.equiv("Host"))
            .map(|h| h.value.as_str());
        if host != Some(&self.host) {
            return Err(Error::Invalid("Use Ripple's 127.0.0.1 address."));
        }
        let url = Url::parse(&format!("{}{}", self.origin, request.url()))
            .map_err(|_| Error::Invalid("Invalid request URL."))?;
        match (request.method(), url.path()) {
            (&Method::Options, "/api/plugin/change") => {
                if request_header(request, "Origin").is_none()
                    || request_header(request, "Access-Control-Request-Method") != Some("POST")
                {
                    return Err(Error::Invalid("Invalid plugin preflight."));
                }
                json_reply(json!({"ok":true}))
            }
            (&Method::Get, "/") => {
                let source = if self.page_path.as_os_str().is_empty() {
                    fs::read(Path::new(ROOT).join("examples/plugin/index.html"))?
                } else {
                    INDEX.as_bytes().to_vec()
                };
                Ok((200, "text/html; charset=utf-8", source, None))
            }
            (&Method::Get, "/api/state") => {
                let project = self
                    .project_path
                    .as_ref()
                    .map(|path| Project::open(path))
                    .transpose()?;
                let page = if self.page_path.as_os_str().is_empty() {
                    None
                } else {
                    Some(read_page(&self.page_path)?)
                };
                let revision = if let Some(page) = &page {
                    page.revision()?
                } else {
                    project
                        .as_ref()
                        .ok_or(Error::Invalid("Missing project."))?
                        .revision()?
                };
                let source = project
                    .as_ref()
                    .map(|p| {
                        if p.manifest.page.is_empty() {
                            p.manifest
                                .entrypoints
                                .first()
                                .map(String::as_str)
                                .unwrap_or("")
                        } else {
                            &p.manifest.page
                        }
                    })
                    .unwrap_or("ripple.page.json");
                json_reply(
                    json!({"mode":if page.is_some() {"document"} else {"plugin"},"revision":revision,"document":page,"sourceFile":source,
                    "project":project.as_ref().map(|p| json!({"sources":p.manifest.sources,"checks":p.manifest.checks.iter().map(|c| &c.name).collect::<Vec<_>>()})),
                    "account":self.accounts.active().map(|c| c.account()), "model":MODEL, "effort":EFFORT}),
                )
            }
            (&Method::Get, "/auth/login") => {
                if self.oauth.is_none() {
                    self.oauth = Some(OAuthClient::discover().await?);
                }
                let new_account = url
                    .query_pairs()
                    .any(|(key, value)| key == "new" && value == "1");
                let returning = if new_account {
                    None
                } else {
                    self.accounts.active().map(|c| c.account())
                };
                let attempt = self.oauth.as_ref().unwrap().begin(
                    self.accounts.host_id(),
                    &format!("{}/auth/callback", self.origin),
                    returning,
                )?;
                let location = attempt.authorization_url.to_string();
                self.pending = Some(attempt);
                Ok((302, "text/plain", vec![], Some(location)))
            }
            (&Method::Get, "/auth/callback") => {
                let attempt = self
                    .pending
                    .take()
                    .ok_or(Error::Invalid("Start a new ChatGPT sign-in from Ripple."))?;
                let oauth = self
                    .oauth
                    .as_ref()
                    .ok_or(Error::Invalid("Start a new ChatGPT sign-in."))?;
                let credentials = oauth.finish(attempt, url.as_str()).await?;
                self.accounts.put(credentials)?;
                Ok((200, "text/html; charset=utf-8", "<!doctype html><meta charset=utf-8><title>Ripple</title><body style='font:18px system-ui;padding:60px;background:#f4f3ed;color:#14271f'><h1>ChatGPT połączony z Ripple.</h1><p>Możesz zamknąć tę kartę i wrócić do edytora.</p><a href='/'>Otwórz Ripple</a>".as_bytes().to_vec(), None))
            }
            (&Method::Post, "/api/change") => {
                let origin = request
                    .headers()
                    .iter()
                    .find(|h| h.field.equiv("Origin"))
                    .map(|h| h.value.as_str());
                let marker = request
                    .headers()
                    .iter()
                    .find(|h| h.field.equiv("X-Ripple-Request"))
                    .map(|h| h.value.as_str());
                if !accepts_change(origin, marker, &self.origin) {
                    return Err(Error::Invalid(
                        "Only Ripple's local editor can submit changes.",
                    ));
                }
                let mut body = vec![];
                request.as_reader().take(3_000_001).read_to_end(&mut body)?;
                if body.len() > 3_000_000 {
                    return Err(Error::Invalid("Visual request is too large."));
                }
                let intent: VisualIntent = serde_json::from_slice(&body)?;
                let current = read_page(&self.page_path)?;
                ensure_revision(&current, &intent.base_revision)?;
                if self.accounts.active().is_none() {
                    return Err(Error::Invalid(
                        "Najpierw wybierz Continue with ChatGPT i połącz subskrypcję.",
                    ));
                }
                if self.accounts.active().unwrap().needs_refresh() {
                    if self.oauth.is_none() {
                        self.oauth = Some(OAuthClient::discover().await?);
                    }
                    let renewed = self
                        .oauth
                        .as_ref()
                        .unwrap()
                        .refresh(self.accounts.active().unwrap())
                        .await?;
                    self.accounts.put(renewed)?;
                }
                let credentials = self.accounts.active().unwrap();
                let catalog = self.models.models(credentials).await?;
                if !catalog.iter().any(|m| m.slug == MODEL) {
                    return Err(Error::Invalid(
                        "GPT-6 Luna is not available to this ChatGPT account.",
                    ));
                }
                if let Some(path) = &self.project_path {
                    let project = Project::open(path)?;
                    if project.page_path() != self.page_path {
                        return Err(Error::Invalid(
                            "Page path changed in the manifest. Restart Ripple.",
                        ));
                    }
                    let page_source = project.manifest.page.clone();
                    let mut workspace = project.begin(&intent.preview, &intent.base_revision)?;
                    let edit = self
                        .models
                        .edit_project(credentials, &current, &intent, &mut workspace)
                        .await?;
                    let changes = workspace.commit()?;
                    let rebuild_required = changes.iter().any(|change| change.path != page_source);
                    let next = read_page(&self.page_path)?;
                    return json_reply(json!({"revision":next.revision()?,"document":next,
                        "summary":edit.summary,"changes":changes,"checks":edit.checks,"rebuildRequired":rebuild_required}));
                }
                let next = self.models.edit(credentials, &current, &intent).await?;
                ensure_revision(&read_page(&self.page_path)?, &intent.base_revision)?;
                atomic_json(&self.page_path, &next)?;
                json_reply(json!({"revision":next.revision()?, "document":next}))
            }
            (&Method::Get, path) if path.starts_with("/modules/") => {
                let parts: Vec<_> = path.split('/').collect();
                if parts.len() != 4
                    || !["core", "harness", "plugin"].contains(&parts[2])
                    || !parts[3].ends_with(".js")
                    || !parts[3]
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || c == b'.' || c == b'-')
                    || parts[3].contains("..")
                {
                    return Err(Error::Invalid("Unknown runtime module."));
                }
                let file = Path::new(ROOT)
                    .join("packages")
                    .join(parts[2])
                    .join("dist")
                    .join(parts[3]);
                let source = fs::read(file)
                    .map_err(|_| Error::Invalid("Run npm run build before starting Ripple."))?;
                Ok((200, "text/javascript", source, None))
            }
            _ => Ok((404, "text/plain", b"Not found".to_vec(), None)),
        }
    }
}

fn request_header<'a>(request: &'a Request, name: &'static str) -> Option<&'a str> {
    request
        .headers()
        .iter()
        .find(|h| h.field.equiv(name))
        .map(|h| h.value.as_str())
}

impl App {
    fn cors_origin(&self, request: &Request) -> Result<Option<String>> {
        if request_header(request, "Host") != Some(&self.host) {
            return Err(Error::Invalid("Use Ripple's 127.0.0.1 address."));
        }
        let Some(origin) = request_header(request, "Origin") else {
            return Ok(None);
        };
        let allowed = origin == self.origin
            || self
                .project_path
                .as_ref()
                .map(|p| Project::open(p))
                .transpose()?
                .is_some_and(|p| {
                    p.manifest
                        .dev_origins
                        .iter()
                        .any(|allowed| allowed == origin)
                });
        if !allowed {
            return Err(Error::Invalid(
                "Origin is not in ripple.project.json devOrigins.",
            ));
        }
        Ok(Some(origin.into()))
    }

    async fn credentials(&mut self) -> Result<Credentials> {
        if self.accounts.active().is_none() {
            return Err(Error::Invalid(
                "Najpierw wybierz Continue with ChatGPT i połącz subskrypcję.",
            ));
        }
        if self.accounts.active().unwrap().needs_refresh() {
            if self.oauth.is_none() {
                self.oauth = Some(OAuthClient::discover().await?);
            }
            let renewed = self
                .oauth
                .as_ref()
                .unwrap()
                .refresh(self.accounts.active().unwrap())
                .await?;
            self.accounts.put(renewed)?;
        }
        let credentials = self.accounts.active().unwrap().clone();
        let subject = credentials.account().subject.clone();
        if !self.catalog.as_ref().is_some_and(|(account, time)| {
            account == &subject && time.elapsed() < Duration::from_secs(300)
        }) {
            if !self
                .models
                .models(&credentials)
                .await?
                .iter()
                .any(|model| model.slug == MODEL)
            {
                return Err(Error::Invalid(
                    "GPT-6 Luna is not available to this ChatGPT account.",
                ));
            }
            self.catalog = Some((subject, Instant::now()));
        }
        Ok(credentials)
    }

    async fn start_plugin(
        &mut self,
        request: &mut Request,
        runtime: &tokio::runtime::Runtime,
    ) -> Result<Job> {
        if self.cors_origin(request)?.is_none()
            || request_header(request, "X-Ripple-Request") != Some("1")
        {
            return Err(Error::Invalid(
                "Only configured local editors can submit changes.",
            ));
        }
        if self.running.swap(true, Ordering::SeqCst) {
            return Err(Error::Invalid(
                "Luna już pracuje. Poczekaj na zakończenie kroku.",
            ));
        }
        let lease = RunLease(self.running.clone());
        let mut body = vec![];
        request.as_reader().take(3_000_001).read_to_end(&mut body)?;
        if body.len() > 3_000_000 {
            return Err(Error::Invalid("Visual request is too large."));
        }
        let intent: PluginIntent = serde_json::from_slice(&body)?;
        intent.validate()?;
        let path = self
            .project_path
            .as_ref()
            .ok_or(Error::Invalid(
                "Plugin requires --project ripple.project.json.",
            ))?
            .clone();
        let project = Project::open(&path)?;
        if !project.manifest.page.is_empty() {
            return Err(Error::Invalid(
                "Remove legacy page from the plugin manifest.",
            ));
        }
        let mut workspace = project.begin_sources(&intent.base_revision)?;
        let credentials = self.credentials().await?;
        let models = self.models.clone();
        let (tx, rx) = mpsc::channel();
        let worker = runtime.spawn(async move {
            let _lease = lease;
            let progress = |event| { let _ = tx.send(json!({"type":"progress","progress":event})); };
            let result: Result<Value> = async {
                let edit = models.edit_quick(&credentials, &intent, &mut workspace, &progress).await?;
                report(&progress, "saving", "Kontrole przeszły · zapisuję pliki", 3);
                let changes = workspace.commit()?;
                let revision = Project::open(&path)?.revision()?;
                let rebuild_required = changes.iter().any(|change| change.path.ends_with(".rs") || matches!(change.path.as_str(), "Cargo.toml" | "Cargo.lock") || change.path.starts_with("packages/"));
                report(&progress, "complete", "Zapisano · gotowe", 4);
                Ok(json!({"revision":revision,"summary":edit.summary,"changes":changes,"checks":edit.checks,"rebuildRequired":rebuild_required}))
            }.await;
            let event = match result { Ok(result) => json!({"type":"complete","result":result}), Err(error) => json!({"type":"error","error":error.to_string()}) };
            let _ = tx.send(event);
        });
        Ok(Job {
            rx,
            worker: worker.abort_handle(),
        })
    }
}

struct RunLease(Arc<AtomicBool>);
impl Drop for RunLease {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}
struct Job {
    rx: mpsc::Receiver<Value>,
    worker: tokio::task::AbortHandle,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.worker.abort();
    }
}

/// Explicit chunk flushes: tiny_http's default encoder buffers small SSE events.
fn stream_job(request: Request, job: Job, origin: Option<String>) {
    let mut writer = request.into_writer();
    let cors = origin
        .map(|origin| format!("Access-Control-Allow-Origin: {origin}\r\nVary: Origin\r\n"))
        .unwrap_or_default();
    let headers = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream; charset=utf-8\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n{cors}\r\n"
    );
    if writer
        .write_all(headers.as_bytes())
        .and_then(|_| writer.flush())
        .is_err()
    {
        return;
    }
    loop {
        let (bytes, terminal) = match job.rx.recv_timeout(Duration::from_secs(1)) {
            Ok(event) => {
                let terminal = matches!(event["type"].as_str(), Some("complete" | "error"));
                (format!("data: {event}\n\n").into_bytes(), terminal)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => (b": keepalive\n\n".to_vec(), false),
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };
        if write!(writer, "{:x}\r\n", bytes.len())
            .and_then(|_| writer.write_all(&bytes))
            .and_then(|_| writer.write_all(b"\r\n"))
            .and_then(|_| writer.flush())
            .is_err()
        {
            return;
        }
        if terminal {
            break;
        }
    }
    let _ = writer.write_all(b"0\r\n\r\n").and_then(|_| writer.flush());
}

fn accepts_change(origin: Option<&str>, marker: Option<&str>, expected: &str) -> bool {
    origin == Some(expected) && marker == Some("1")
}
fn ensure_revision(page: &Page, expected: &str) -> Result<()> {
    if page.revision()? != expected {
        return Err(Error::Invalid(
            "Plik strony zmienił się. Odśwież stronę przed wysłaniem kolejnej zmiany.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_accepts_editor_requests_from_the_expected_origin() {
        assert!(accepts_change(
            Some("http://127.0.0.1:4310"),
            Some("1"),
            "http://127.0.0.1:4310"
        ));
        assert!(!accepts_change(
            Some("https://other.example"),
            Some("1"),
            "http://127.0.0.1:4310"
        ));
        assert!(!accepts_change(None, Some("1"), "http://127.0.0.1:4310"));
        assert!(!accepts_change(
            Some("http://127.0.0.1:4310"),
            None,
            "http://127.0.0.1:4310"
        ));
    }

    #[test]
    fn refuses_to_overwrite_an_external_edit() {
        let mut page: Page = serde_json::from_value(
            json!({"version":1,"title":"First","root":{"id":"root","tag":"main"}}),
        )
        .unwrap();
        let revision = page.revision().unwrap();
        ensure_revision(&page, &revision).unwrap();
        page.title = "External edit".into();
        assert!(ensure_revision(&page, &revision).is_err());
    }
    #[tokio::test]
    async fn sse_flushes_progress_before_completion_and_uses_exact_cors_origin() {
        use std::{
            io::{BufRead, BufReader},
            net::TcpStream,
        };
        let server = Server::http("127.0.0.1:0").unwrap();
        let address = server.server_addr().to_ip().unwrap();
        let (tx, rx) = mpsc::channel();
        let worker = tokio::spawn(std::future::pending::<()>());
        let job = Job {
            rx,
            worker: worker.abort_handle(),
        };
        let thread = std::thread::spawn(move || {
            stream_job(
                server.recv().unwrap(),
                job,
                Some("http://127.0.0.1:5173".into()),
            )
        });
        let mut socket = TcpStream::connect(address).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        write!(
            socket,
            "POST /api/plugin/change HTTP/1.1\r\nHost: {address}\r\nContent-Length: 0\r\n\r\n"
        )
        .unwrap();
        let mut reader = BufReader::new(socket);
        let mut headers = String::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            headers.push_str(&line);
            if line == "\r\n" {
                break;
            }
        }
        assert!(headers.contains("Access-Control-Allow-Origin: http://127.0.0.1:5173\r\n"));
        assert!(headers.contains("Transfer-Encoding: chunked\r\n"));
        tx.send(json!({"type":"progress","progress":{"phase":"model","label":"Luna"}}))
            .unwrap();
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let size = usize::from_str_radix(line.trim(), 16).unwrap();
        let mut bytes = vec![0; size + 2];
        reader.read_exact(&mut bytes).unwrap();
        assert!(String::from_utf8(bytes).unwrap().contains("Luna"));
        // Completion is deliberately sent only after the first event was received.
        tx.send(json!({"type":"complete","result":{"revision":"new"}}))
            .unwrap();
        let mut tail = String::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let size = usize::from_str_radix(line.trim(), 16).unwrap();
            if size == 0 {
                break;
            }
            let mut bytes = vec![0; size + 2];
            reader.read_exact(&mut bytes).unwrap();
            tail.push_str(&String::from_utf8(bytes).unwrap());
        }
        assert!(tail.contains("complete") && tail.contains("new"));
        thread.join().unwrap();
        assert!(worker.await.unwrap_err().is_cancelled());
    }
}
