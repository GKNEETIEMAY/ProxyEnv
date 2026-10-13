use super::BridgeResult;
use std::sync::{Mutex, OnceLock};
use zeroize::{Zeroize, Zeroizing};

pub struct ProtectedPassword(Zeroizing<Vec<u8>>);

struct CacheEntry {
    fingerprint: String,
    password: ProtectedPassword,
}

static CACHE: OnceLock<Mutex<Option<CacheEntry>>> = OnceLock::new();

fn cache() -> &'static Mutex<Option<CacheEntry>> {
    CACHE.get_or_init(|| Mutex::new(None))
}

fn lock() -> BridgeResult<std::sync::MutexGuard<'static, Option<CacheEntry>>> {
    cache().lock().map_err(|_| "stateUnavailable".into())
}

pub fn protect(password: &[u8], fingerprint: &str) -> BridgeResult<ProtectedPassword> {
    protect_for_current_user(password, fingerprint.as_bytes()).map(ProtectedPassword)
}

pub fn store(fingerprint: String, password: ProtectedPassword) -> BridgeResult<()> {
    *lock()? = Some(CacheEntry {
        fingerprint,
        password,
    });
    Ok(())
}

pub fn contains(fingerprint: &str) -> bool {
    lock()
        .ok()
        .and_then(|entry| entry.as_ref().map(|entry| entry.fingerprint == fingerprint))
        .unwrap_or(false)
}

pub fn reveal(fingerprint: &str) -> BridgeResult<Option<Zeroizing<Vec<u8>>>> {
    let cache = lock()?;
    let Some(entry) = cache
        .as_ref()
        .filter(|entry| entry.fingerprint == fingerprint)
    else {
        return Ok(None);
    };
    unprotect_for_current_user(&entry.password.0, fingerprint.as_bytes()).map(Some)
}

#[cfg(windows)]
pub fn terminal_payload(fingerprint: &str) -> BridgeResult<Option<(String, String)>> {
    let cache = lock()?;
    let Some(entry) = cache
        .as_ref()
        .filter(|entry| entry.fingerprint == fingerprint)
    else {
        return Ok(None);
    };
    Ok(Some((
        hex::encode(entry.password.0.as_slice()),
        hex::encode(fingerprint.as_bytes()),
    )))
}

pub fn clear() {
    if let Ok(mut cache) = cache().lock() {
        *cache = None;
    }
}

pub fn clear_if_matches(fingerprint: &str) {
    if let Ok(mut cache) = cache().lock() {
        if cache
            .as_ref()
            .is_some_and(|entry| entry.fingerprint == fingerprint)
        {
            *cache = None;
        }
    }
}

#[cfg(windows)]
fn protect_for_current_user(data: &[u8], entropy: &[u8]) -> BridgeResult<Zeroizing<Vec<u8>>> {
    use windows::{
        core::w,
        Win32::Security::Cryptography::{
            CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
        },
    };

    let input = blob(data)?;
    let entropy = blob(entropy)?;
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptProtectData(
            &input,
            w!("ProxyEnv SSH bridge password"),
            Some(&entropy),
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
        .map_err(|_| "credentialProtectionFailed")?;
    }
    take_local_blob(output, false)
}

#[cfg(windows)]
fn unprotect_for_current_user(data: &[u8], entropy: &[u8]) -> BridgeResult<Zeroizing<Vec<u8>>> {
    use windows::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    let input = blob(data)?;
    let entropy = blob(entropy)?;
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptUnprotectData(
            &input,
            None,
            Some(&entropy),
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
        .map_err(|_| "credentialProtectionFailed")?;
    }
    take_local_blob(output, true)
}

#[cfg(windows)]
fn blob(data: &[u8]) -> BridgeResult<windows::Win32::Security::Cryptography::CRYPT_INTEGER_BLOB> {
    use windows::Win32::Security::Cryptography::CRYPT_INTEGER_BLOB;

    Ok(CRYPT_INTEGER_BLOB {
        cbData: data
            .len()
            .try_into()
            .map_err(|_| "credentialProtectionFailed")?,
        pbData: data.as_ptr().cast_mut(),
    })
}

#[cfg(windows)]
fn take_local_blob(
    output: windows::Win32::Security::Cryptography::CRYPT_INTEGER_BLOB,
    wipe: bool,
) -> BridgeResult<Zeroizing<Vec<u8>>> {
    use windows::Win32::Foundation::{LocalFree, HLOCAL};

    if output.pbData.is_null() || output.cbData == 0 {
        return Err("credentialProtectionFailed".into());
    }
    let mut value = Zeroizing::new(unsafe {
        std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec()
    });
    if wipe {
        unsafe { std::ptr::write_bytes(output.pbData, 0, output.cbData as usize) };
    }
    unsafe {
        let _ = LocalFree(Some(HLOCAL(output.pbData.cast())));
    }
    if value.is_empty() {
        value.zeroize();
        return Err("credentialProtectionFailed".into());
    }
    Ok(value)
}

#[cfg(not(windows))]
fn protect_for_current_user(_data: &[u8], _entropy: &[u8]) -> BridgeResult<Zeroizing<Vec<u8>>> {
    Err("credentialProtectionUnavailable".into())
}

#[cfg(not(windows))]
fn unprotect_for_current_user(_data: &[u8], _entropy: &[u8]) -> BridgeResult<Zeroizing<Vec<u8>>> {
    Err("credentialProtectionUnavailable".into())
}

#[cfg(windows)]
pub fn try_run_askpass() -> bool {
    if std::env::var_os("PROXYENV_SSH_ASKPASS").as_deref() != Some(std::ffi::OsStr::new("1")) {
        return false;
    }
    let prompt = std::env::args().nth(1).unwrap_or_default().to_lowercase();
    if !is_reusable_password_prompt(&prompt) {
        std::process::exit(1);
    }
    let result = (|| {
        let protected = hex::decode(std::env::var("PROXYENV_SSH_PASSWORD")?)?;
        let entropy = hex::decode(std::env::var("PROXYENV_SSH_PASSWORD_ENTROPY")?)?;
        let password = unprotect_for_current_user(&protected, &entropy)
            .map_err(|_| "unable to decrypt credential")?;
        use std::io::Write;
        std::io::stdout().write_all(&password)?;
        Ok::<(), Box<dyn std::error::Error>>(())
    })();
    std::process::exit(if result.is_ok() { 0 } else { 1 });
}

#[cfg(windows)]
fn is_reusable_password_prompt(prompt: &str) -> bool {
    prompt.contains("password")
        && !prompt.contains("passphrase")
        && !prompt.contains("verification")
        && !prompt.contains("one-time")
        && !prompt.contains("otp")
}

#[cfg(not(windows))]
pub fn try_run_askpass() -> bool {
    false
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn dpapi_round_trip_is_bound_to_fingerprint() {
        let password = b"temporary-password";
        let protected = protect(password, "target-a").unwrap();
        let revealed = unprotect_for_current_user(&protected.0, b"target-a").unwrap();
        assert_eq!(revealed.as_slice(), password);
        assert!(unprotect_for_current_user(&protected.0, b"target-b").is_err());
    }

    #[test]
    fn cache_is_target_scoped_and_clearable() {
        clear();
        let protected = protect(b"temporary-password", "target-a").unwrap();
        store("target-a".into(), protected).unwrap();
        assert!(contains("target-a"));
        assert!(!contains("target-b"));
        clear_if_matches("target-b");
        assert!(contains("target-a"));
        clear_if_matches("target-a");
        assert!(!contains("target-a"));
    }

    #[test]
    fn askpass_only_accepts_plain_password_prompts() {
        assert!(is_reusable_password_prompt("user@example's password:"));
        assert!(!is_reusable_password_prompt("enter passphrase for key:"));
        assert!(!is_reusable_password_prompt("one-time password:"));
        assert!(!is_reusable_password_prompt("verification code:"));
    }
}
