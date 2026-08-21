use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS},
        System::Registry::{
            RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
            REG_DWORD, REG_VALUE_TYPE,
        },
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyEndpoint {
    pub host: String,
    pub port: u16,
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

fn open_internet_settings() -> Option<RegistryKey> {
    let path = wide("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings");
    let mut key = HKEY::default();
    let result = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(path.as_ptr()),
            None,
            KEY_READ,
            &mut key,
        )
    };
    (result == ERROR_SUCCESS).then_some(RegistryKey(key))
}

fn read_raw(key: HKEY, name: &str) -> Option<(REG_VALUE_TYPE, Vec<u8>)> {
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
    if result == ERROR_FILE_NOT_FOUND || result != ERROR_SUCCESS {
        return None;
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
    (result == ERROR_SUCCESS).then_some((kind, data))
}

fn read_dword(key: HKEY, name: &str) -> Option<u32> {
    let (kind, data) = read_raw(key, name)?;
    if kind != REG_DWORD || data.len() < 4 {
        return None;
    }
    Some(u32::from_le_bytes(data[..4].try_into().ok()?))
}

fn read_string(key: HKEY, name: &str) -> Option<String> {
    let (_, data) = read_raw(key, name)?;
    let words = data
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|word| *word != 0)
        .collect::<Vec<_>>();
    Some(String::from_utf16_lossy(&words))
}

pub fn read() -> Option<ProxyEndpoint> {
    let key = open_internet_settings()?;
    if read_dword(key.0, "ProxyEnable")? == 0 {
        return None;
    }
    parse_proxy_server(&read_string(key.0, "ProxyServer")?)
}

pub fn parse_proxy_server(value: &str) -> Option<ProxyEndpoint> {
    let selected = if value.contains('=') {
        value
            .split(';')
            .filter_map(|part| part.split_once('='))
            .find(|(scheme, _)| scheme.eq_ignore_ascii_case("http"))
            .or_else(|| {
                value
                    .split(';')
                    .filter_map(|part| part.split_once('='))
                    .next()
            })?
            .1
    } else {
        value
    };
    let address = selected
        .trim()
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .trim_start_matches("socks5://")
        .split('/')
        .next()?;
    let (host, port) = if let Some(rest) = address.strip_prefix('[') {
        let (host, port) = rest.split_once("]:")?;
        (host, port)
    } else {
        address.rsplit_once(':')?
    };
    let host = match host.trim().to_ascii_lowercase().as_str() {
        "localhost" | "0.0.0.0" => "127.0.0.1".into(),
        "::" => "::1".into(),
        host => host.into(),
    };
    if host != "127.0.0.1" && host != "::1" {
        return None;
    }
    Some(ProxyEndpoint {
        host,
        port: port.trim().parse().ok()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_and_protocol_specific_values() {
        assert_eq!(
            parse_proxy_server("127.0.0.1:7897"),
            Some(ProxyEndpoint {
                host: "127.0.0.1".into(),
                port: 7897
            })
        );
        assert_eq!(
            parse_proxy_server("socks=127.0.0.1:10808;http=127.0.0.1:10809"),
            Some(ProxyEndpoint {
                host: "127.0.0.1".into(),
                port: 10809
            })
        );
    }
}
