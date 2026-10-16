//! Project-wide edits in a disposable copy. This is a work area, not an OS sandbox.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::{NamedTempFile, TempDir};
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::{Error, Result, document::Page};

const MAX_FILE: usize = 512_000;
const MAX_FILES: usize = 512;
const MAX_TOTAL: usize = 8_000_000;
const MAX_LOG: usize = 16_000;
type Files = BTreeMap<String, String>;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ProjectManifest {
    pub version: u8,
    pub page: String,
    pub sources: Vec<String>,
    pub checks: Vec<Check>,
    #[serde(default)]
    pub instructions: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Check {
    pub name: String,
    /// An executable and its arguments, never a shell command string.
    pub command: Vec<String>,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    pub name: String,
    pub success: bool,
    pub timed_out: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Serialize)]
pub struct FileChange {
    pub path: String,
    pub kind: &'static str,
}

pub struct Project {
    root: PathBuf,
    manifest_path: String,
    manifest_text: String,
    pub manifest: ProjectManifest,
}

impl Project {
    pub fn open(path: &Path) -> Result<Self> {
        if fs::symlink_metadata(path)?.file_type().is_symlink() {
            return Err(Error::Invalid("Project manifest cannot be a symlink."));
        }
        let path = fs::canonicalize(path)?;
        let root = path
            .parent()
            .ok_or(Error::Invalid("Missing project root."))?
            .to_path_buf();
        let manifest_path = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(Error::Invalid("Invalid project manifest name."))?
            .to_owned();
        let manifest_text = read_text(&path)?;
        let manifest: ProjectManifest = serde_json::from_str(&manifest_text)?;
        if manifest.version != 1
            || manifest.sources.is_empty()
            || manifest.sources.len() > 64
            || manifest.checks.is_empty()
            || manifest.checks.len() > 8
            || manifest.instructions.len() > 20_000
        {
            return Err(Error::Invalid(
                "Invalid project manifest or missing checks.",
            ));
        }
        let mut names = BTreeSet::new();
        for check in &manifest.checks {
            if check.name.is_empty()
                || check.name.len() > 80
                || !names.insert(&check.name)
                || check.command.is_empty()
                || check.command.len() > 32
                || check
                    .command
                    .iter()
                    .any(|arg| arg.is_empty() || arg.len() > 2_000 || arg.contains('\0'))
                || !(1..=120).contains(&check.timeout_seconds)
            {
                return Err(Error::Invalid("Invalid or duplicate project check."));
            }
        }
        valid_path(&manifest.page)?;
        for source in &manifest.sources {
            valid_path(source)?;
            if source == &manifest_path {
                return Err(Error::Invalid(
                    "The agent cannot edit its project manifest.",
                ));
            }
        }
        if !allowed(&manifest, &manifest.page) {
            return Err(Error::Invalid("The page must be inside project sources."));
        }
        let project = Self {
            root,
            manifest_path,
            manifest_text,
            manifest,
        };
        project.page()?.validate()?;
        Ok(project)
    }

    pub fn page_path(&self) -> PathBuf {
        self.root.join(&self.manifest.page)
    }

    pub fn page(&self) -> Result<Page> {
        no_symlinks(&self.root, &self.manifest.page)?;
        let page: Page = serde_json::from_str(&read_text(&self.page_path())?)?;
        page.validate()?;
        Ok(page)
    }

    pub fn begin(self, preview: &Page, expected_revision: &str) -> Result<Workspace> {
        let originals = snapshot(&self.root, &self.manifest)?;
        let original_page: Page = serde_json::from_str(
            originals
                .get(&self.manifest.page)
                .ok_or(Error::Invalid("Missing project page."))?,
        )?;
        if original_page.revision()? != expected_revision {
            return Err(Error::Invalid(
                "Page changed before the project run. Refresh the editor.",
            ));
        }
        let temp = tempfile::tempdir()?;
        let stage = fs::canonicalize(temp.path())?;
        for (path, content) in &originals {
            write_text(&stage, path, content)?;
            fs::set_permissions(
                stage.join(path),
                fs::metadata(self.root.join(path))?.permissions(),
            )?;
        }
        write_text(&stage, &self.manifest_path, &self.manifest_text)?;
        link_dependencies(&self.root, &stage)?;
        let mut workspace = Workspace {
            project: self,
            _temp: temp,
            stage,
            originals,
            checks: BTreeMap::new(),
        };
        preview.validate()?;
        if original_page.revision()? != preview.revision()? {
            let path = workspace.project.manifest.page.clone();
            workspace.write(
                &path,
                &format!("{}\n", serde_json::to_string_pretty(preview)?),
            )?;
        }
        Ok(workspace)
    }
}

pub struct Workspace {
    project: Project,
    _temp: TempDir,
    stage: PathBuf,
    originals: Files,
    checks: BTreeMap<String, (String, CheckResult)>,
}

impl Workspace {
    pub fn context(&self) -> Result<Value> {
        Ok(
            json!({"manifest":self.project.manifest, "files":self.files()?.keys().collect::<Vec<_>>()}),
        )
    }

    fn files(&self) -> Result<Files> {
        snapshot(&self.stage, &self.project.manifest)
    }

    fn source(&self, path: &str) -> Result<()> {
        valid_path(path)?;
        if path == self.project.manifest_path || !allowed(&self.project.manifest, path) {
            return Err(Error::Invalid(
                "Path is outside configured project sources.",
            ));
        }
        no_symlinks(&self.stage, path)
    }

    pub fn read(&self, path: &str) -> Result<String> {
        self.source(path)?;
        read_text(&self.stage.join(path))
    }

    pub fn write(&mut self, path: &str, content: &str) -> Result<()> {
        self.source(path)?;
        if content.len() > MAX_FILE {
            return Err(Error::Invalid("Source file is too large."));
        }
        // Check limits before changing the stage, including newly created files.
        let mut files = self.files()?;
        files.insert(path.into(), content.into());
        check_limits(&files)?;
        write_text(&self.stage, path, content)
    }

    fn delete(&mut self, path: &str) -> Result<()> {
        self.source(path)?;
        if path == self.project.manifest.page {
            return Err(Error::Invalid("Cannot delete the active page."));
        }
        fs::remove_file(self.stage.join(path))?;
        Ok(())
    }

    pub async fn tool(&mut self, name: &str, arguments: &str) -> Result<Value> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Empty {}
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct PathArg {
            path: String,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct WriteArg {
            path: String,
            content: String,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct CheckArg {
            name: String,
        }
        match name {
            "list_files" => {
                let _: Empty = serde_json::from_str(arguments)?;
                Ok(json!({"files":self.files()?.keys().collect::<Vec<_>>()}))
            }
            "read_file" => {
                let args: PathArg = serde_json::from_str(arguments)?;
                Ok(json!({"path":args.path, "content":self.read(&args.path)?}))
            }
            "write_file" => {
                let args: WriteArg = serde_json::from_str(arguments)?;
                self.write(&args.path, &args.content)?;
                Ok(json!({"staged":args.path}))
            }
            "delete_file" => {
                let args: PathArg = serde_json::from_str(arguments)?;
                self.delete(&args.path)?;
                Ok(json!({"deleted":args.path}))
            }
            "run_check" => {
                let args: CheckArg = serde_json::from_str(arguments)?;
                Ok(serde_json::to_value(self.run_check(&args.name).await?)?)
            }
            _ => Err(Error::Invalid("Unknown project tool.")),
        }
    }

    pub async fn run_check(&mut self, name: &str) -> Result<CheckResult> {
        let check = self
            .project
            .manifest
            .checks
            .iter()
            .find(|check| check.name == name)
            .ok_or(Error::Invalid("Unknown configured check."))?
            .clone();
        let fingerprint = fingerprint(&self.files()?);
        let result = run(&check, &self.stage, &self.project.root).await?;
        self.checks
            .insert(name.into(), (fingerprint, result.clone()));
        Ok(result)
    }

    /// Checks are cached only for the exact source content they tested.
    pub async fn verify(&mut self) -> Result<Vec<CheckResult>> {
        let page: Page = serde_json::from_str(&self.read(&self.project.manifest.page)?)?;
        page.validate()?;
        let names: Vec<_> = self
            .project
            .manifest
            .checks
            .iter()
            .map(|check| check.name.clone())
            .collect();
        let mut results = vec![];
        for name in &names {
            let current = fingerprint(&self.files()?);
            let result = match self.checks.get(name) {
                Some((revision, result)) if revision == &current => result.clone(),
                _ => self.run_check(name).await?,
            };
            results.push(result);
        }
        if results.iter().any(|check| !check.success) {
            return Err(Error::Validation(serde_json::to_string(&results)?));
        }
        if !self.is_verified()? {
            return Err(Error::Validation(
                "Checks changed source files; run checks again on the final sources.".into(),
            ));
        }
        Ok(results)
    }

    fn is_verified(&self) -> Result<bool> {
        let current = fingerprint(&self.files()?);
        Ok(self.project.manifest.checks.iter().all(|check| {
            self.checks
                .get(&check.name)
                .is_some_and(|(revision, result)| revision == &current && result.success)
        }))
    }

    /// All original sources and the frozen check configuration must still match.
    pub fn commit(self) -> Result<Vec<FileChange>> {
        if !self.is_verified()? {
            return Err(Error::Invalid(
                "Run project checks before applying changes.",
            ));
        }
        no_symlinks(&self.project.root, &self.project.manifest_path)?;
        if read_text(&self.project.root.join(&self.project.manifest_path))?
            != self.project.manifest_text
            || snapshot(&self.project.root, &self.project.manifest)? != self.originals
        {
            return Err(Error::Invalid(
                "Projekt zmienił się podczas pracy Luny. Wyślij zmianę ponownie.",
            ));
        }
        let files = self.files()?;
        let page: Page = serde_json::from_str(
            files
                .get(&self.project.manifest.page)
                .ok_or(Error::Invalid("Missing project page."))?,
        )?;
        page.validate()?;
        let paths: BTreeSet<_> = files.keys().chain(self.originals.keys()).cloned().collect();
        let changed: Vec<_> = paths
            .into_iter()
            .filter(|path| files.get(path) != self.originals.get(path))
            .collect();
        // Preflight every path before the first write; writes use same-directory renames.
        for path in &changed {
            no_symlinks(&self.project.root, path)?;
        }
        let mut applied: Vec<String> = vec![];
        for path in &changed {
            if let Err(error) = replace(&self.project.root, path, files.get(path)) {
                for applied_path in applied.iter().rev() {
                    if replace(
                        &self.project.root,
                        applied_path,
                        self.originals.get(applied_path),
                    )
                    .is_err()
                    {
                        return Err(Error::Invalid(
                            "Project write and rollback failed. Check your working tree.",
                        ));
                    }
                }
                return Err(error);
            }
            applied.push(path.clone());
        }
        Ok(changed
            .into_iter()
            .map(|path| {
                let kind = if !files.contains_key(&path) {
                    "deleted"
                } else if self.originals.contains_key(&path) {
                    "modified"
                } else {
                    "created"
                };
                FileChange { path, kind }
            })
            .collect())
    }
}

fn allowed(manifest: &ProjectManifest, path: &str) -> bool {
    manifest
        .sources
        .iter()
        .any(|source| Path::new(path).starts_with(source))
}

fn valid_path(path: &str) -> Result<()> {
    if path.is_empty()
        || path.len() > 400
        || path.contains(['\\', ':', '\0'])
        || path
            .split('/')
            .any(|part| part.is_empty() || part.starts_with('.') || blocked(part))
        || !Path::new(path)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
    {
        return Err(Error::Invalid(
            "Invalid, hidden, generated, or private source path.",
        ));
    }
    Ok(())
}

fn blocked(part: &str) -> bool {
    let part = part.to_ascii_lowercase();
    matches!(
        part.as_str(),
        "node_modules" | "target" | "dist" | "coverage" | "accounts.json"
    ) || [".pem", ".key", ".sqlite", ".sqlite3", ".db"]
        .iter()
        .any(|suffix| part.ends_with(suffix))
}

fn no_symlinks(root: &Path, path: &str) -> Result<()> {
    let mut current = root.to_path_buf();
    for part in Path::new(path).components() {
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(Error::Invalid("Source paths cannot contain symlinks."));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn read_text(path: &Path) -> Result<String> {
    let mut bytes = vec![];
    fs::File::open(path)?
        .take((MAX_FILE + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_FILE {
        return Err(Error::Invalid("Source file is too large."));
    }
    String::from_utf8(bytes).map_err(|_| Error::Invalid("Project sources must be UTF-8 text."))
}

fn snapshot(root: &Path, manifest: &ProjectManifest) -> Result<Files> {
    fn visit(root: &Path, path: &str, files: &mut Files) -> Result<()> {
        valid_path(path)?;
        no_symlinks(root, path)?;
        let full = root.join(path);
        let metadata = match fs::symlink_metadata(&full) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        if metadata.is_dir() {
            for entry in fs::read_dir(full)? {
                let name = entry?
                    .file_name()
                    .into_string()
                    .map_err(|_| Error::Invalid("Non-UTF-8 source path."))?;
                if name.starts_with('.') || blocked(&name) {
                    continue;
                }
                visit(root, &format!("{path}/{name}"), files)?;
            }
        } else if metadata.is_file() {
            files.insert(path.into(), read_text(&full)?);
            check_limits(files)?;
        } else {
            return Err(Error::Invalid("Only regular source files are supported."));
        }
        Ok(())
    }
    let mut files = BTreeMap::new();
    for path in &manifest.sources {
        visit(root, path, &mut files)?;
    }
    Ok(files)
}

fn check_limits(files: &Files) -> Result<()> {
    if files.len() > MAX_FILES || files.values().map(String::len).sum::<usize>() > MAX_TOTAL {
        return Err(Error::Invalid(
            "Project sources exceed 512 files or 8 MB. Narrow sources in the manifest.",
        ));
    }
    Ok(())
}

fn fingerprint(files: &Files) -> String {
    let mut digest = Sha256::new();
    for (path, content) in files {
        digest.update((path.len() as u64).to_le_bytes());
        digest.update(path.as_bytes());
        digest.update((content.len() as u64).to_le_bytes());
        digest.update(content.as_bytes());
    }
    URL_SAFE_NO_PAD.encode(digest.finalize())
}

fn write_text(root: &Path, path: &str, content: &str) -> Result<()> {
    no_symlinks(root, path)?;
    let path = root.join(path);
    let parent = path
        .parent()
        .ok_or(Error::Invalid("Missing source directory."))?;
    fs::create_dir_all(parent)?;
    let mut temp = NamedTempFile::new_in(parent)?;
    if let Ok(metadata) = fs::metadata(&path) {
        temp.as_file().set_permissions(metadata.permissions())?;
    }
    temp.write_all(content.as_bytes())?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|error| error.error)?;
    Ok(())
}

fn replace(root: &Path, path: &str, content: Option<&String>) -> Result<()> {
    no_symlinks(root, path)?;
    match content {
        Some(content) => write_text(root, path, content),
        None => {
            fs::remove_file(root.join(path))?;
            Ok(())
        }
    }
}

/// Reuse installed packages; workspace package links must point into the stage.
#[cfg(unix)]
fn link_dependencies(root: &Path, stage: &Path) -> Result<()> {
    use std::os::unix::fs::symlink;
    fn link(root: &Path, stage: &Path, relative: &Path) -> Result<()> {
        let directory = root.join(relative);
        if !directory.is_dir() {
            return Ok(());
        }
        fs::create_dir_all(stage.join(relative))?;
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let name = entry.file_name();
            let relative = relative.join(&name);
            if name.to_string_lossy().starts_with('@') {
                link(root, stage, &relative)?;
            } else {
                let target = fs::canonicalize(entry.path())?;
                let target = target
                    .strip_prefix(root)
                    .ok()
                    .filter(|relative| !relative.starts_with("node_modules"))
                    .map_or(target.clone(), |relative| stage.join(relative));
                symlink(target, stage.join(relative))?;
            }
        }
        Ok(())
    }
    link(root, stage, Path::new("node_modules"))
}

#[cfg(not(unix))]
fn link_dependencies(_root: &Path, _stage: &Path) -> Result<()> {
    Ok(())
}

async fn capture(mut reader: impl AsyncRead + Unpin) -> Vec<u8> {
    let mut output = vec![];
    let mut buffer = [0; 4096];
    while let Ok(count) = reader.read(&mut buffer).await {
        if count == 0 {
            break;
        }
        let keep = count.min(MAX_LOG.saturating_sub(output.len()));
        output.extend_from_slice(&buffer[..keep]);
    }
    output
}

struct ProcessGroup(Option<u32>);

impl Drop for ProcessGroup {
    fn drop(&mut self) {
        #[cfg(not(unix))]
        let _ = self.0;
        #[cfg(unix)]
        if let Some(pid) = self.0 {
            // SAFETY: this group was created for the child, never our own group.
            unsafe {
                libc::kill(-(pid as i32), libc::SIGKILL);
            }
        }
    }
}

async fn run(check: &Check, stage: &Path, root: &Path) -> Result<CheckResult> {
    let mut command = Command::new(&check.command[0]);
    command
        .args(&check.command[1..])
        .current_dir(stage)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for name in [
        "PATH",
        "HOME",
        "RUSTUP_HOME",
        "CARGO_HOME",
        "TMPDIR",
        "SystemRoot",
        "USERPROFILE",
    ] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    command.env("CARGO_TARGET_DIR", root.join("target"));
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut command = tokio::process::Command::from(command);
    command.kill_on_drop(true);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return Ok(CheckResult {
                name: check.name.clone(),
                success: false,
                timed_out: false,
                exit_code: None,
                stdout: String::new(),
                stderr: error.to_string(),
            });
        }
    };
    let group = ProcessGroup(child.id());
    let stdout = tokio::spawn(capture(child.stdout.take().unwrap()));
    let stderr = tokio::spawn(capture(child.stderr.take().unwrap()));
    let status =
        tokio::time::timeout(Duration::from_secs(check.timeout_seconds), child.wait()).await;
    // Terminate the entire group, including descendants holding log pipes open.
    drop(group);
    let timed_out = status.is_err();
    if timed_out {
        let _ = child.kill().await;
    }
    let exit_code = match status {
        Ok(status) => status?.code(),
        Err(_) => None,
    };
    let mut outputs = vec![];
    for mut task in [stdout, stderr] {
        let output = match tokio::time::timeout(Duration::from_secs(2), &mut task).await {
            Ok(Ok(output)) => output,
            _ => {
                task.abort();
                vec![]
            }
        };
        outputs.push(String::from_utf8_lossy(&output).into_owned());
    }
    Ok(CheckResult {
        name: check.name.clone(),
        success: !timed_out && exit_code == Some(0),
        timed_out,
        exit_code,
        stdout: outputs.remove(0),
        stderr: outputs.remove(0),
    })
}

#[cfg(all(test, unix))]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn fixture(command: &[&str]) -> (TempDir, Page) {
        let root = tempfile::tempdir().unwrap();
        let page: Page = serde_json::from_value(
            json!({"version":1,"title":"Notes","root":{"id":"root","tag":"main"}}),
        )
        .unwrap();
        for (path, text) in [
            ("server/main.rs", "original"),
            ("web/app.ts", "original"),
            ("migrations/001.sql", "original"),
            ("web/obsolete.ts", "old"),
        ] {
            write_text(root.path(), path, text).unwrap();
        }
        write_text(
            root.path(),
            "ripple.page.json",
            &serde_json::to_string(&page).unwrap(),
        )
        .unwrap();
        let manifest = json!({"version":1,"page":"ripple.page.json","sources":["server","web","migrations","ripple.page.json"],
            "checks":[{"name":"test","command":command,"timeoutSeconds":1}]});
        write_text(root.path(), "ripple.project.json", &manifest.to_string()).unwrap();
        (root, page)
    }

    fn begin(root: &TempDir, page: &Page) -> Workspace {
        Project::open(&root.path().join("ripple.project.json"))
            .unwrap()
            .begin(page, &page.revision().unwrap())
            .unwrap()
    }

    #[tokio::test]
    async fn edits_all_layers_in_a_copy_then_publishes_only_after_checks() {
        let (root, page) = fixture(&[
            "sh",
            "-c",
            "test -s server/main.rs && test -s web/app.ts && test -s migrations/002.sql",
        ]);
        let mut workspace = begin(&root, &page);
        workspace.write("server/main.rs", "new backend").unwrap();
        workspace.write("web/app.ts", "new frontend").unwrap();
        workspace
            .write("migrations/002.sql", "CREATE TABLE notes (id INTEGER);")
            .unwrap();
        workspace.delete("web/obsolete.ts").unwrap();
        assert_eq!(
            fs::read_to_string(root.path().join("server/main.rs")).unwrap(),
            "original"
        );
        assert!(!root.path().join("migrations/002.sql").exists());
        assert!(workspace.verify().await.unwrap()[0].success);
        let changes = workspace.commit().unwrap();
        assert_eq!(changes.len(), 4);
        assert_eq!(
            fs::read_to_string(root.path().join("server/main.rs")).unwrap(),
            "new backend"
        );
        assert!(root.path().join("migrations/002.sql").exists());
        assert!(!root.path().join("web/obsolete.ts").exists());
    }

    #[tokio::test]
    async fn failed_check_and_dropped_run_leave_originals_untouched() {
        let (root, page) = fixture(&["sh", "-c", "echo broken >&2; exit 7"]);
        let mut workspace = begin(&root, &page);
        workspace.write("server/main.rs", "broken").unwrap();
        let result = workspace.run_check("test").await.unwrap();
        assert!(!result.success);
        assert_eq!(result.exit_code, Some(7));
        assert!(result.stderr.contains("broken"));
        assert!(workspace.verify().await.is_err());
        assert!(workspace.commit().is_err());
        assert_eq!(
            fs::read_to_string(root.path().join("server/main.rs")).unwrap(),
            "original"
        );
    }

    #[tokio::test]
    async fn a_later_edit_invalidates_successful_checks() {
        let (root, page) = fixture(&["sh", "-c", "test \"$(cat server/main.rs)\" = original"]);
        let mut workspace = begin(&root, &page);
        workspace.verify().await.unwrap();
        workspace.write("server/main.rs", "unchecked").unwrap();
        assert!(workspace.verify().await.is_err());
        assert!(workspace.commit().is_err());
    }

    #[tokio::test]
    async fn conflicts_in_any_source_or_manifest_abort_before_writing() {
        for conflict in ["web/app.ts", "ripple.project.json"] {
            let (root, page) = fixture(&["true"]);
            let mut workspace = begin(&root, &page);
            workspace.write("server/main.rs", "model edit").unwrap();
            workspace.verify().await.unwrap();
            fs::write(root.path().join(conflict), "external edit").unwrap();
            assert!(workspace.commit().is_err());
            assert_eq!(
                fs::read_to_string(root.path().join("server/main.rs")).unwrap(),
                "original"
            );
            assert_eq!(
                fs::read_to_string(root.path().join(conflict)).unwrap(),
                "external edit"
            );
        }
    }

    #[tokio::test]
    async fn limits_paths_commands_and_symlinks_including_new_parent_directories() {
        use std::os::unix::fs::symlink;
        let (root, page) = fixture(&["true"]);
        fs::write(root.path().join("server/.env"), "secret").unwrap();
        let mut workspace = begin(&root, &page);
        for path in [
            "../escape",
            "/tmp/escape",
            "server/../escape",
            "server/.env",
            "server/accounts.json",
            "server/db.sqlite",
            "server/node_modules/pkg",
            "Cargo.toml",
            "ripple.project.json",
        ] {
            assert!(workspace.read(path).is_err(), "{path}");
            assert!(workspace.write(path, "bad").is_err(), "{path}");
        }
        assert!(
            workspace
                .tool("run_check", r#"{"name":"arbitrary","command":["sh"]}"#)
                .await
                .is_err()
        );
        assert!(workspace.delete("ripple.page.json").is_err());
        assert!(
            workspace
                .write("web/large.ts", &"x".repeat(MAX_FILE + 1))
                .is_err()
        );
        let outside = tempfile::tempdir().unwrap();
        workspace.write("server/new/module.rs", "new").unwrap();
        workspace.verify().await.unwrap();
        symlink(outside.path(), root.path().join("server/new")).unwrap();
        assert!(workspace.commit().is_err());
        assert!(!outside.path().join("module.rs").exists());
    }

    #[tokio::test]
    async fn rejects_invalid_page_and_bounds_command_time_and_logs() {
        let (root, page) = fixture(&[
            "sh",
            "-c",
            "while :; do printf abcdefghijklmnopqrstuvwxyz; done",
        ]);
        let mut workspace = begin(&root, &page);
        let result = workspace.run_check("test").await.unwrap();
        assert!(result.timed_out);
        assert!(!result.success);
        assert_eq!(result.stdout.len(), MAX_LOG);
        workspace
            .write(
                "ripple.page.json",
                r#"{"version":1,"title":"Bad","root":{"id":"root","tag":"script"}}"#,
            )
            .unwrap();
        assert!(workspace.verify().await.is_err());
        assert!(workspace.commit().is_err());
    }

    #[test]
    fn rewrites_installed_workspace_package_links_into_the_work_area() {
        use std::os::unix::fs::symlink;
        let (root, page) = fixture(&["true"]);
        fs::create_dir_all(root.path().join("node_modules/@app")).unwrap();
        symlink(
            root.path().join("web"),
            root.path().join("node_modules/@app/web"),
        )
        .unwrap();
        let workspace = begin(&root, &page);
        assert_eq!(
            fs::canonicalize(workspace.stage.join("node_modules/@app/web")).unwrap(),
            workspace.stage.join("web")
        );
    }

    #[tokio::test]
    async fn cancelling_a_check_terminates_its_descendants() {
        let (root, page) = fixture(&[
            "sh",
            "-c",
            "(printf started > server/started; sleep 1; printf survived > server/survived) & wait",
        ]);
        let workspace = begin(&root, &page);
        let check = workspace.project.manifest.checks[0].clone();
        let stage = workspace.stage.clone();
        let project_root = workspace.project.root.clone();
        let task = tokio::spawn(async move { run(&check, &stage, &project_root).await });
        for _ in 0..100 {
            if workspace.stage.join("server/started").exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(workspace.stage.join("server/started").exists());
        task.abort();
        let _ = task.await;
        tokio::time::sleep(Duration::from_millis(1_100)).await;
        assert!(!workspace.stage.join("server/survived").exists());
    }
}
