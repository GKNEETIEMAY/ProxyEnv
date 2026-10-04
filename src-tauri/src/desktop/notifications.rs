//! No target names, ports or credentials in desktop notifications.
#![allow(dead_code)] // Dormant until reconnect notifications return to the product surface.
use crate::services::settings;
use std::sync::OnceLock;
use tauri::{AppHandle, Emitter, Manager};
static APP: OnceLock<AppHandle> = OnceLock::new();
pub fn setup(app: AppHandle) {
    let _ = APP.set(app);
}

pub fn bridge_attention() {
    let Some(app) = APP.get() else {
        return;
    };
    let preferences = settings::load().unwrap_or_default();
    if !preferences.notifications_enabled {
        return;
    }
    let focused = app
        .get_webview_window("main")
        .is_some_and(|w| w.is_focused().unwrap_or(false));
    if focused {
        return;
    }
    #[cfg(windows)]
    {
        let sound = preferences.notification_sound;
        let language = preferences.resolved_language();
        let _ = native_toast(app, language, sound);
    }
    // Permission or OS policy may suppress desktop toasts. The bridge page
    // remains the single in-app source of recovery status.
}

#[cfg(windows)]
fn native_toast(app: &AppHandle, language: &str, sound: bool) -> windows::core::Result<()> {
    use windows::{
        core::HSTRING,
        Data::Xml::Dom::XmlDocument,
        Foundation::TypedEventHandler,
        Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED},
        UI::Notifications::{ToastNotification, ToastNotificationManager},
    };
    // Called on the bridge monitor thread. The apartment lives with that thread.
    thread_local! { static INITIALIZED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
    INITIALIZED.with(|flag| -> windows::core::Result<()> {
        if !flag.get() {
            unsafe {
                RoInitialize(RO_INIT_MULTITHREADED)?;
            }
            flag.set(true);
        }
        Ok(())
    })?;
    let (title, body) = match language {
        "zh-CN" => (
            "桥接需要处理",
            "自动重连已暂停。点击返回 ProxyEnv 检查认证或连接设置。",
        ),
        "ja" => (
            "ブリッジの確認が必要です",
            "自動再接続を一時停止しました。クリックして認証や接続設定を確認してください。",
        ),
        "ko" => (
            "브리지 확인 필요",
            "자동 재연결이 일시 중지되었습니다. 클릭하여 인증 또는 연결 설정을 확인하세요.",
        ),
        _ => (
            "Bridge needs attention",
            "Automatic reconnection paused. Click to review authentication or connection settings.",
        ),
    };
    let audio = if sound {
        ""
    } else {
        "<audio silent=\"true\"/>"
    };
    let document = XmlDocument::new()?;
    document.LoadXml(&HSTRING::from(format!("<toast><visual><binding template=\"ToastGeneric\"><text>ProxyEnv · {title}</text><text>{body}</text></binding></visual>{audio}</toast>")))?;
    let toast = ToastNotification::CreateToastNotification(&document)?;
    toast.SetTag(&HSTRING::from("bridge-attention"))?;
    let handle = app.clone();
    toast.Activated(&TypedEventHandler::new(move |_, _| {
        super::tray::show_main_window(&handle);
        let _ = handle.emit("bridge-notification-open", ());
        Ok(())
    }))?;
    ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(
        app.config().identifier.clone(),
    ))?
    .Show(&toast)?;
    // Keep activation handler alive; bounded to one notification, no history.
    static CURRENT: std::sync::Mutex<Option<ToastNotification>> = std::sync::Mutex::new(None);
    if let Ok(mut current) = CURRENT.lock() {
        *current = Some(toast);
    }
    Ok(())
}
