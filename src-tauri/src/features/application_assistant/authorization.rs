use std::{
    collections::HashMap,
    fs::Metadata,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt as _;
#[cfg(windows)]
use std::os::windows::fs::MetadataExt as _;

use crate::error::{ProxyEnvError, Result};

use super::{launcher, ManagedApplication, RunningApplication};

const AUTHORIZATION_TTL: Duration = Duration::from_secs(10 * 60);
const MAX_AUTHORIZATIONS: usize = 256;
const TOKEN_BYTES: usize = 32;

#[derive(Clone)]
struct AuthorizedApplication {
    application: ManagedApplication,
    identity: ExecutableIdentity,
    expires_at: Instant,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExecutableIdentity {
    canonical_path: PathBuf,
    file_size: u64,
    #[cfg(windows)]
    creation_time: u64,
    #[cfg(windows)]
    last_write_time: u64,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    modified_seconds: i64,
    #[cfg(unix)]
    modified_nanoseconds: i64,
}

#[derive(Default)]
struct AuthorizationStore {
    applications: HashMap<String, AuthorizedApplication>,
}

impl AuthorizationStore {
    fn purge_expired(&mut self, now: Instant) {
        self.applications
            .retain(|_, authorization| authorization.expires_at > now);
    }

    fn insert(
        &mut self,
        application: ManagedApplication,
        identity: ExecutableIdentity,
        now: Instant,
    ) {
        self.purge_expired(now);
        if self.applications.len() >= MAX_AUTHORIZATIONS {
            if let Some(oldest) = self
                .applications
                .iter()
                .min_by_key(|(_, authorization)| authorization.expires_at)
                .map(|(id, _)| id.clone())
            {
                self.applications.remove(&oldest);
            }
        }
        self.applications.insert(
            application.id.clone(),
            AuthorizedApplication {
                application,
                identity,
                expires_at: now + AUTHORIZATION_TTL,
            },
        );
    }

    fn resolve(&mut self, application_id: &str, now: Instant) -> Option<AuthorizedApplication> {
        self.purge_expired(now);
        let authorization = self.applications.get_mut(application_id)?;
        authorization.expires_at = now + AUTHORIZATION_TTL;
        Some(authorization.clone())
    }

    fn remove(&mut self, application_id: &str) {
        self.applications.remove(application_id);
    }
}

fn store() -> &'static Mutex<AuthorizationStore> {
    static STORE: OnceLock<Mutex<AuthorizationStore>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(AuthorizationStore::default()))
}

pub fn authorize_running_applications(
    applications: Vec<RunningApplication>,
) -> Vec<RunningApplication> {
    applications
        .into_iter()
        .map(|mut application| {
            application.application_id = application
                .executable_path
                .as_deref()
                .and_then(|path| authorize(path, &application.display_name).ok())
                .map(|authorized| authorized.id);
            application
        })
        .collect()
}

pub fn authorize_path(path: &Path) -> Result<ManagedApplication> {
    let display_name = application_display_name(path);
    authorize(path, &display_name)
}

fn authorize(path: &Path, display_name: &str) -> Result<ManagedApplication> {
    let executable_path = launcher::validate_executable(path)?;
    let identity = executable_identity(&executable_path)?;
    let mut registry = store().lock().map_err(|_| {
        ProxyEnvError::ApplicationAuthorization("the authorization store is unavailable".into())
    })?;
    let id = unique_token(&registry)?;
    let application = ManagedApplication {
        id,
        display_name: display_name.trim().to_owned(),
        executable_path,
        icon_key: None,
        rule_id: None,
        last_action: None,
    };
    registry.insert(application.clone(), identity, Instant::now());
    Ok(application)
}

pub fn resolve_application(application_id: &str) -> Result<ManagedApplication> {
    validate_token(application_id)?;
    let mut registry = store().lock().map_err(|_| {
        ProxyEnvError::ApplicationAuthorization("the authorization store is unavailable".into())
    })?;
    let authorization = registry
        .resolve(application_id, Instant::now())
        .ok_or_else(|| {
            ProxyEnvError::ApplicationAuthorization(
                "the application authorization is missing or expired; refresh the application list"
                    .into(),
            )
        })?;
    let current_path = launcher::validate_executable(&authorization.application.executable_path)?;
    let current_identity = executable_identity(&current_path)?;
    if current_identity != authorization.identity {
        registry.remove(application_id);
        return Err(ProxyEnvError::ApplicationAuthorization(
            "the authorized executable changed; refresh the application list".into(),
        ));
    }
    Ok(authorization.application)
}

fn executable_identity(path: &Path) -> Result<ExecutableIdentity> {
    let canonical_path = path
        .canonicalize()
        .map_err(|error| ProxyEnvError::InvalidApplication(error.to_string()))?;
    let metadata = canonical_path
        .metadata()
        .map_err(|error| ProxyEnvError::InvalidApplication(error.to_string()))?;
    Ok(identity_from_metadata(canonical_path, &metadata))
}

fn identity_from_metadata(canonical_path: PathBuf, metadata: &Metadata) -> ExecutableIdentity {
    ExecutableIdentity {
        canonical_path,
        file_size: metadata.len(),
        #[cfg(windows)]
        creation_time: metadata.creation_time(),
        #[cfg(windows)]
        last_write_time: metadata.last_write_time(),
        #[cfg(unix)]
        device: metadata.dev(),
        #[cfg(unix)]
        inode: metadata.ino(),
        #[cfg(unix)]
        modified_seconds: metadata.mtime(),
        #[cfg(unix)]
        modified_nanoseconds: metadata.mtime_nsec(),
    }
}

fn unique_token(registry: &AuthorizationStore) -> Result<String> {
    for _ in 0..4 {
        let token = random_token()?;
        if !registry.applications.contains_key(&token) {
            return Ok(token);
        }
    }
    Err(ProxyEnvError::ApplicationAuthorization(
        "could not allocate a unique application authorization".into(),
    ))
}

fn random_token() -> Result<String> {
    let mut bytes = [0_u8; TOKEN_BYTES];
    getrandom::fill(&mut bytes).map_err(|error| {
        ProxyEnvError::ApplicationAuthorization(format!(
            "the operating system random generator failed: {error}"
        ))
    })?;
    Ok(hex::encode(bytes))
}

fn validate_token(token: &str) -> Result<()> {
    if token.len() != TOKEN_BYTES * 2 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ProxyEnvError::ApplicationAuthorization(
            "the application authorization id is invalid".into(),
        ));
    }
    Ok(())
}

fn application_display_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("Application")
        .replace(['-', '_'], " ")
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn application(id: &str) -> ManagedApplication {
        ManagedApplication {
            id: id.into(),
            display_name: "Example".into(),
            executable_path: PathBuf::from("example.exe"),
            icon_key: None,
            rule_id: None,
            last_action: None,
        }
    }

    #[test]
    fn generated_tokens_are_randomly_distinct_and_well_formed() {
        let first = random_token().unwrap();
        let second = random_token().unwrap();
        assert_ne!(first, second);
        assert!(validate_token(&first).is_ok());
        assert!(validate_token("running:C:/Apps/example.exe").is_err());
    }

    #[test]
    fn an_authorized_executable_resolves_to_its_canonical_path() {
        let executable = std::env::current_exe().unwrap();
        let authorized = authorize_path(&executable).unwrap();
        let resolved = resolve_application(&authorized.id).unwrap();
        assert_eq!(resolved.executable_path, executable.canonicalize().unwrap());
    }

    #[test]
    fn expired_authorizations_are_not_resolved() {
        let now = Instant::now();
        let token = "a".repeat(TOKEN_BYTES * 2);
        let mut store = AuthorizationStore::default();
        store.applications.insert(
            token.clone(),
            AuthorizedApplication {
                application: application(&token),
                identity: executable_identity(&std::env::current_exe().unwrap()).unwrap(),
                expires_at: now,
            },
        );
        assert!(store.resolve(&token, now).is_none());
        assert!(store.applications.is_empty());
    }

    #[test]
    fn resolving_an_authorization_extends_its_lifetime() {
        let now = Instant::now();
        let token = "b".repeat(TOKEN_BYTES * 2);
        let original_expiry = now + Duration::from_secs(1);
        let mut store = AuthorizationStore::default();
        store.applications.insert(
            token.clone(),
            AuthorizedApplication {
                application: application(&token),
                identity: executable_identity(&std::env::current_exe().unwrap()).unwrap(),
                expires_at: original_expiry,
            },
        );

        assert!(store.resolve(&token, now).is_some());
        assert_eq!(
            store.applications.get(&token).unwrap().expires_at,
            now + AUTHORIZATION_TTL
        );
        assert!(store.applications.get(&token).unwrap().expires_at > original_expiry);
    }

    #[test]
    fn derives_a_readable_name_from_a_selected_path() {
        assert_eq!(
            application_display_name(Path::new(r"C:\Apps\Example-Desktop.exe")),
            "Example Desktop"
        );
    }
}
