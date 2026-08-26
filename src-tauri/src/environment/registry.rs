use crate::error::{ProxyEnvError, Result};

use super::{EnvironmentEntry, EnvironmentMutation, EnvironmentScope};

#[cfg(windows)]
mod platform {
    use super::*;
    use windows::{
        core::PCWSTR,
        Win32::{
            Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR},
            System::Registry::*,
        },
    };

    fn registry_error(action: &str, error: WIN32_ERROR) -> String {
        format!("{action} failed with Win32 error {}", error.0)
    }

    struct RegistryKey(HKEY);

    impl Drop for RegistryKey {
        fn drop(&mut self) {
            unsafe {
                let _ = RegCloseKey(self.0);
            }
        }
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn open(access: REG_SAM_FLAGS) -> Result<RegistryKey> {
        let mut key = HKEY::default();
        let path = wide("Environment");
        let result = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(path.as_ptr()),
                None,
                access,
                &mut key,
            )
        };
        if result != ERROR_SUCCESS {
            return Err(ProxyEnvError::RegistryRead(registry_error(
                "open HKCU\\Environment",
                result,
            )));
        }
        Ok(RegistryKey(key))
    }

    fn read_value(key: HKEY, name: &str) -> Result<Option<String>> {
        let name = wide(name);
        let mut kind = REG_VALUE_TYPE::default();
        let mut size = 0_u32;
        let result = unsafe {
            RegQueryValueExW(
                key,
                PCWSTR(name.as_ptr()),
                None,
                Some(&mut kind),
                None,
                Some(&mut size),
            )
        };
        if result == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        if result != ERROR_SUCCESS {
            return Err(ProxyEnvError::RegistryRead(registry_error(
                "query environment value size",
                result,
            )));
        }
        let mut data = vec![0_u8; size as usize];
        let result = unsafe {
            RegQueryValueExW(
                key,
                PCWSTR(name.as_ptr()),
                None,
                Some(&mut kind),
                Some(data.as_mut_ptr()),
                Some(&mut size),
            )
        };
        if result != ERROR_SUCCESS {
            return Err(ProxyEnvError::RegistryRead(registry_error(
                "read environment value",
                result,
            )));
        }
        let words = data
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .take_while(|word| *word != 0)
            .collect::<Vec<_>>();
        Ok(Some(String::from_utf16_lossy(&words)))
    }

    pub fn read_entries(
        names: &[String],
        scope: EnvironmentScope,
    ) -> Result<Vec<EnvironmentEntry>> {
        let key = open(KEY_READ)?;
        names
            .iter()
            .map(|name| {
                let value = read_value(key.0, name)?;
                Ok(EnvironmentEntry {
                    name: name.clone(),
                    exists: value.is_some(),
                    value,
                    scope,
                })
            })
            .collect()
    }

    pub fn apply_mutations(
        mutations: &[EnvironmentMutation],
        _scope: EnvironmentScope,
    ) -> Result<()> {
        let key = open(KEY_SET_VALUE)?;
        for mutation in mutations {
            let name = mutation.name();
            let name_wide = wide(name);
            match mutation {
                EnvironmentMutation::Set { value, .. } => {
                    let value_wide = wide(value);
                    let bytes = unsafe {
                        std::slice::from_raw_parts(
                            value_wide.as_ptr().cast::<u8>(),
                            value_wide.len() * 2,
                        )
                    };
                    let result = unsafe {
                        RegSetValueExW(key.0, PCWSTR(name_wide.as_ptr()), None, REG_SZ, Some(bytes))
                    };
                    if result != ERROR_SUCCESS {
                        return Err(ProxyEnvError::RegistryWrite(registry_error(
                            "write environment value",
                            result,
                        )));
                    }
                }
                EnvironmentMutation::Delete { .. } => {
                    let result = unsafe { RegDeleteValueW(key.0, PCWSTR(name_wide.as_ptr())) };
                    if result != ERROR_SUCCESS && result != ERROR_FILE_NOT_FOUND {
                        return Err(ProxyEnvError::RegistryWrite(registry_error(
                            "delete environment value",
                            result,
                        )));
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(not(windows))]
mod platform {
    use super::*;
    pub fn read_entries(_: &[String], _: EnvironmentScope) -> Result<Vec<EnvironmentEntry>> {
        Err(ProxyEnvError::UnsupportedPlatform)
    }
    pub fn apply_mutations(_: &[EnvironmentMutation], _: EnvironmentScope) -> Result<()> {
        Err(ProxyEnvError::UnsupportedPlatform)
    }
}

pub use platform::*;
