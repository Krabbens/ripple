use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    Error, Result,
    oauth::{Credentials, is_legacy_host_id, is_valid_host_id, new_host_id},
};

#[derive(Clone, Deserialize, Serialize)]
struct StoreData {
    version: u8,
    host_id: String,
    active_client_id: Option<String>,
    accounts: Vec<Credentials>,
}

/// Holds an exclusive process lock for the lifetime of the store, including renewal.
pub struct AccountStore {
    root: PathBuf,
    data: StoreData,
    lock: File,
}

impl Drop for AccountStore {
    fn drop(&mut self) {
        // Forked check processes may briefly inherit this open-file description.
        // Unlock explicitly rather than waiting for the last duplicate to close.
        let _ = self.lock.unlock();
    }
}

impl AccountStore {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        if fs::symlink_metadata(&root)?.file_type().is_symlink() {
            return Err(Error::Invalid("The account directory cannot be a symlink."));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
            let lock_path = root.join("accounts.lock");
            reject_symlink(&lock_path)?;
            let lock = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .mode(0o600)
                .open(lock_path)?;
            Self::load(root, lock)
        }
        #[cfg(not(unix))]
        {
            let lock = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(root.join("accounts.lock"))?;
            Self::load(root, lock)
        }
    }

    fn load(root: PathBuf, lock: File) -> Result<Self> {
        lock.try_lock().map_err(|_| Error::StoreLocked)?;
        let path = root.join("accounts.json");
        reject_symlink(&path)?;
        let data = if path.exists() {
            let mut bytes = Vec::new();
            File::open(&path)?.take(2_000_001).read_to_end(&mut bytes)?;
            if bytes.len() > 2_000_000 {
                return Err(Error::Invalid("Account store is too large."));
            }
            let mut data: StoreData = serde_json::from_slice(&bytes)?;
            if data.version != 1 || data.host_id.is_empty() {
                return Err(Error::Invalid("Invalid account store."));
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
            }
            if migrate_host_identity(&mut data)? {
                atomic_json(&path, &data)?;
            }
            data
        } else {
            let data = StoreData {
                version: 1,
                host_id: new_host_id(),
                active_client_id: None,
                accounts: vec![],
            };
            atomic_json(&path, &data)?;
            data
        };
        Ok(Self { root, data, lock })
    }

    pub fn host_id(&self) -> &str {
        &self.data.host_id
    }

    pub fn active(&self) -> Option<&Credentials> {
        self.data
            .accounts
            .iter()
            .find(|c| Some(&c.account.client_id) == self.data.active_client_id.as_ref())
    }

    pub fn put(&mut self, credentials: Credentials) -> Result<()> {
        if credentials.account.host_id != self.data.host_id {
            return Err(Error::Invalid("Account host does not match."));
        }
        let mut data = self.data.clone();
        if let Some(previous) = data
            .accounts
            .iter_mut()
            .find(|c| c.account.client_id == credentials.account.client_id)
        {
            if previous.account.subject != credentials.account.subject
                || previous.account.issuer != credentials.account.issuer
            {
                return Err(Error::Invalid(
                    "Account registration cannot change identity.",
                ));
            }
            *previous = credentials.clone();
        } else {
            data.accounts.push(credentials.clone());
        }
        data.active_client_id = Some(credentials.account.client_id);
        atomic_json(&self.root.join("accounts.json"), &data)?;
        self.data = data;
        Ok(())
    }
}

fn migrate_host_identity(data: &mut StoreData) -> Result<bool> {
    if is_valid_host_id(&data.host_id) {
        return Ok(false);
    }
    if !is_legacy_host_id(&data.host_id)
        || data
            .accounts
            .iter()
            .any(|c| c.account.host_id != data.host_id)
    {
        return Err(Error::Invalid(
            "Invalid host identity in the account store.",
        ));
    }
    let host_id = new_host_id();
    for credentials in &mut data.accounts {
        credentials.account.host_id = host_id.clone();
    }
    data.host_id = host_id;
    Ok(true)
}

fn reject_symlink(path: &Path) -> Result<()> {
    if path
        .symlink_metadata()
        .is_ok_and(|m| m.file_type().is_symlink())
    {
        return Err(Error::Invalid("Credential files cannot be symlinks."));
    }
    Ok(())
}

pub fn atomic_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    file.write_all(b"\n")?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|e| Error::Io(e.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oauth::Account;

    #[test]
    fn migrates_legacy_host_once_without_losing_registered_accounts_or_tokens() {
        let dir = tempfile::tempdir().unwrap();
        let legacy = crate::oauth::random_value().unwrap();
        let value = serde_json::json!({
            "version":1,"host_id":legacy,"active_client_id":"existing-client",
            "accounts":[{"account":{"issuer":"https://auth.openai.com","subject":"existing-user","client_id":"existing-client","host_id":legacy,"email":null},
            "access_token":"existing-access","refresh_token":"existing-refresh","id_token":"existing-id",
            "scopes":["chatgpt.tokens.use.direct"],"expires_at":u64::MAX}]
        });
        atomic_json(&dir.path().join("accounts.json"), &value).unwrap();
        let store = AccountStore::open(dir.path()).unwrap();
        let host = store.host_id().to_owned();
        assert!(is_valid_host_id(&host));
        assert_ne!(host, legacy);
        let credentials = store.active().unwrap();
        assert_eq!(credentials.account.client_id, "existing-client");
        assert_eq!(credentials.account.host_id, host);
        assert_eq!(credentials.access_token, "existing-access");
        assert_eq!(credentials.refresh_token, "existing-refresh");
        drop(store);
        assert_eq!(AccountStore::open(dir.path()).unwrap().host_id(), host);
    }

    #[test]
    fn rejects_unknown_host_corruption_without_rewriting_the_store() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("accounts.json");
        let value = serde_json::json!({"version":1,"host_id":"not-a-host","active_client_id":null,"accounts":[]});
        atomic_json(&path, &value).unwrap();
        let previous = fs::read(&path).unwrap();
        assert!(AccountStore::open(dir.path()).is_err());
        assert_eq!(fs::read(&path).unwrap(), previous);
    }

    #[test]
    fn persists_host_identity_and_locks_out_a_second_process() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = AccountStore::open(dir.path()).unwrap();
        let host = store.host_id().to_owned();
        assert!(matches!(
            AccountStore::open(dir.path()),
            Err(Error::StoreLocked)
        ));
        let account = Account {
            issuer: "https://auth.openai.com".into(),
            subject: "test-user".into(),
            client_id: "test-client".into(),
            host_id: host.clone(),
            email: None,
        };
        store
            .put(Credentials {
                account,
                access_token: "test-access".into(),
                refresh_token: "test-refresh".into(),
                id_token: "test-id".into(),
                scopes: vec!["chatgpt.tokens.use.direct".into()],
                expires_at: u64::MAX,
            })
            .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(dir.path().join("accounts.json"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        drop(store);
        let store = AccountStore::open(dir.path()).unwrap();
        assert_eq!(store.host_id(), host);
        assert_eq!(store.active().unwrap().account().subject, "test-user");
    }

    #[cfg(unix)]
    #[test]
    fn dropping_the_store_releases_a_lock_even_with_an_inherited_duplicate() {
        let dir = tempfile::tempdir().unwrap();
        let store = AccountStore::open(dir.path()).unwrap();
        let inherited = store.lock.try_clone().unwrap();
        drop(store);
        let _next = AccountStore::open(dir.path()).unwrap();
        drop(inherited);
        assert!(matches!(
            AccountStore::open(dir.path()),
            Err(Error::StoreLocked)
        ));
    }
}
