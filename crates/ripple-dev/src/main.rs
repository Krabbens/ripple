use std::{
    env, fs,
    io::Read,
    path::{Path, PathBuf},
};

use reqwest::Url;
use ripple_core::{
    Error, Result,
    document::Page,
    oauth::{AuthAttempt, OAuthClient},
    provider::{EFFORT, MODEL, ModelClient, VisualIntent},
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
        page_path = Project::open(path)?.page_path();
    }
    read_page(&page_path)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    if check_only {
        let project = Project::open(project_path.as_ref().ok_or("--check requires --project")?)?;
        let page = project.page()?;
        let mut workspace = project.begin(&page, &page.revision()?)?;
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
    };
    println!("Ripple → {origin}\nGPT-6 Luna · low\nCtrl+C to stop.");
    for mut request in server.incoming_requests() {
        let result = runtime.block_on(app.handle(&mut request));
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
            (&Method::Get, "/") => Ok((
                200,
                "text/html; charset=utf-8",
                INDEX.as_bytes().to_vec(),
                None,
            )),
            (&Method::Get, "/api/state") => {
                let page = read_page(&self.page_path)?;
                let project = self
                    .project_path
                    .as_ref()
                    .map(|path| Project::open(path))
                    .transpose()?;
                json_reply(json!({"revision":page.revision()?, "document":page,
                    "sourceFile":project.as_ref().map(|p| p.manifest.page.as_str()).unwrap_or_else(|| self.page_path.file_name().and_then(|p| p.to_str()).unwrap_or("ripple.page.json")),
                    "project":project.as_ref().map(|p| json!({"sources":p.manifest.sources,"checks":p.manifest.checks.iter().map(|c| &c.name).collect::<Vec<_>>()})),
                    "account":self.accounts.active().map(|c| c.account()), "model":MODEL, "effort":EFFORT}))
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
                    || !["core", "harness"].contains(&parts[2])
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
}
