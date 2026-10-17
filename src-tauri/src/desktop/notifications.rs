//! Optional desktop delivery of existing bridge state. No probes or private payloads.
use crate::{
    features::remote_bridge::tray_state,
    services::{desktop_alerts, settings},
};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    mpsc, OnceLock,
};
use tauri::{AppHandle, Manager};

static WAKE: OnceLock<mpsc::SyncSender<()>> = OnceLock::new();
static ACTIVE: AtomicU64 = AtomicU64::new(0);

pub fn wake() {
    if let Some(sender) = WAKE.get() {
        let _ = sender.try_send(());
    }
}

pub fn setup(app: AppHandle) {
    let (sender, receiver) = mpsc::sync_channel(1);
    if WAKE.set(sender.clone()).is_err() {
        return;
    }
    tray_state::listen(sender);
    std::thread::spawn(move || {
        let mut episode = Episode::default();
        native::clear(&app);
        // Subscribe first, then read the latest cache; changes are coalesced, never polled.
        loop {
            let preferences = settings::load().ok();
            let allowed = preferences
                .as_ref()
                .is_some_and(|p| p.notifications_enabled)
                && desktop_alerts::load().unwrap_or(false);
            // Unknown focus/permission state must be silent, not treated as background.
            let background = app
                .get_webview_window("main")
                .is_some_and(|window| window.is_focused().is_ok_and(|focused| !focused));
            let decision = episode.advance(
                tray_state::snapshot().attention_generation,
                allowed,
                background,
            );
            if decision.clear {
                ACTIVE.store(0, Ordering::SeqCst);
                native::clear(&app);
            }
            if let Some((serial, generation)) = decision.show {
                if let Some(preferences) = preferences {
                    ACTIVE.store(serial, Ordering::SeqCst);
                    let sound = preferences.notification_sound;
                    native::show(
                        &app,
                        preferences.resolved_language(),
                        sound,
                        serial,
                        generation,
                    );
                }
            }
            if receiver.recv().is_err() {
                break;
            }
        }
    });
}

#[derive(Default)]
struct Episode {
    generation: Option<u64>,
    serial: u64,
    delivered: bool,
}
struct Decision {
    clear: bool,
    show: Option<(u64, u64)>,
}

impl Episode {
    fn advance(&mut self, generation: Option<u64>, allowed: bool, background: bool) -> Decision {
        let changed = self.generation != generation;
        if changed {
            self.generation = generation;
            self.serial = self.serial.wrapping_add(1).max(1);
            self.delivered = false;
        }
        let show = generation
            .filter(|_| allowed && background && !self.delivered)
            .map(|generation| (self.serial, generation));
        if show.is_some() {
            self.delivered = true;
        }
        Decision {
            clear: changed || !allowed || !background,
            show,
        }
    }
}

#[cfg(windows)]
mod native {
    use super::*;
    use std::sync::Mutex;
    use tauri::Emitter;
    use windows::{
        core::HSTRING,
        Data::Xml::Dom::XmlDocument,
        Foundation::TypedEventHandler,
        Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED},
        UI::Notifications::{ToastNotification, ToastNotificationManager, ToastNotifier},
    };

    const TAG: &str = "bridge-attention";
    const GROUP: &str = "routespan-bridge";
    struct Current {
        toast: ToastNotification,
        notifier: ToastNotifier,
        activation: i64,
    }
    static CURRENT: Mutex<Option<Current>> = Mutex::new(None);

    fn initialize() -> windows::core::Result<()> {
        thread_local! { static INITIALIZED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
        INITIALIZED.with(|flag| {
            if !flag.get() {
                unsafe {
                    RoInitialize(RO_INIT_MULTITHREADED)?;
                }
                flag.set(true);
            }
            Ok(())
        })
    }

    pub(super) fn clear(app: &AppHandle) {
        if initialize().is_err() {
            return;
        }
        let previous = CURRENT.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(previous) = previous {
            let _ = previous.toast.RemoveActivated(previous.activation);
            let _ = previous.notifier.Hide(&previous.toast);
        }
        if let Ok(history) = ToastNotificationManager::History() {
            let _ = history.RemoveGroupedTagWithId(
                &HSTRING::from(TAG),
                &HSTRING::from(GROUP),
                &HSTRING::from(app.config().identifier.clone()),
            );
            // Also remove the ungrouped toast left by older builds, never other app history.
            let _ = history.RemoveGroupedTagWithId(
                &HSTRING::from(TAG),
                &HSTRING::new(),
                &HSTRING::from(app.config().identifier.clone()),
            );
        }
    }

    pub(super) fn show(app: &AppHandle, language: &str, sound: bool, serial: u64, generation: u64) {
        // OS policy or permission failure must never interrupt the bridge or trigger retries.
        let _ = show_inner(app, language, sound, serial, generation);
    }

    fn show_inner(
        app: &AppHandle,
        language: &str,
        sound: bool,
        serial: u64,
        generation: u64,
    ) -> windows::core::Result<()> {
        initialize()?;
        let (title, body) = match language {
            "zh-CN" => ("桥接需要处理", "自动重连已暂停。点击返回 RouteSpan 检查认证或连接设置。"),
            "ja" => ("ブリッジの確認が必要です", "自動再接続を一時停止しました。クリックして認証や接続設定を確認してください。"),
            "ko" => ("브리지 확인 필요", "자동 재연결이 일시 중지되었습니다. 클릭하여 인증 또는 연결 설정을 확인하세요."),
            _ => ("Bridge needs attention", "Automatic reconnection paused. Click to review authentication or connection settings."),
        };
        let audio = if sound {
            ""
        } else {
            "<audio silent=\"true\"/>"
        };
        let document = XmlDocument::new()?;
        document.LoadXml(&HSTRING::from(format!("<toast><visual><binding template=\"ToastGeneric\"><text>RouteSpan · {title}</text><text>{body}</text></binding></visual>{audio}</toast>")))?;
        let toast = ToastNotification::CreateToastNotification(&document)?;
        toast.SetTag(&HSTRING::from(TAG))?;
        toast.SetGroup(&HSTRING::from(GROUP))?;
        let handle = app.clone();
        let activation = toast.Activated(&TypedEventHandler::new(move |_, _| {
            // A stale click cannot reopen recovery after disconnect/recovery or opt-out.
            if ACTIVE.load(Ordering::SeqCst) == serial
                && tray_state::snapshot().attention_generation == Some(generation)
                && settings::load().is_ok_and(|p| p.notifications_enabled)
                && desktop_alerts::load().unwrap_or(false)
            {
                super::super::tray::show_main_window(&handle);
                let _ = handle.emit("bridge-notification-open", ());
            }
            Ok(())
        }))?;
        let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(
            app.config().identifier.clone(),
        ))?;
        // Revalidate after OS calls: recovery, opt-out or focus may have changed meanwhile.
        if tray_state::snapshot().attention_generation != Some(generation)
            || !settings::load().is_ok_and(|p| p.notifications_enabled)
            || !desktop_alerts::load().unwrap_or(false)
            || !app
                .get_webview_window("main")
                .is_some_and(|window| window.is_focused().is_ok_and(|focused| !focused))
        {
            return Ok(());
        }
        notifier.Show(&toast)?;
        *CURRENT.lock().unwrap_or_else(|e| e.into_inner()) = Some(Current {
            toast,
            notifier,
            activation,
        });
        Ok(())
    }
}

#[cfg(not(windows))]
mod native {
    use super::*;
    pub(super) fn clear(_: &AppHandle) {}
    pub(super) fn show(_: &AppHandle, _: &str, _: bool, _: u64, _: u64) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_delivery_requires_consent_background_and_manual_attention() {
        let mut episode = Episode::default();
        assert!(episode.advance(None, true, true).show.is_none());
        assert!(episode.advance(Some(7), false, true).show.is_none());
        assert!(episode.advance(Some(7), true, false).show.is_none());
        assert_eq!(episode.advance(Some(7), true, true).show, Some((1, 7)));
        for _ in 0..5 {
            assert!(episode.advance(Some(7), true, true).show.is_none());
        }
    }

    #[test]
    fn recovery_disconnect_opt_out_and_focus_clear_without_replaying_old_events() {
        let mut episode = Episode::default();
        episode.advance(Some(3), true, true);
        assert!(episode.advance(Some(3), false, true).clear);
        assert!(episode.advance(Some(3), true, true).show.is_none());
        assert!(episode.advance(Some(3), true, false).clear);
        assert!(episode.advance(Some(3), true, true).show.is_none());
        assert!(episode.advance(None, true, true).clear);
        assert_eq!(episode.advance(Some(4), true, true).show, Some((3, 4)));
        assert!(episode.advance(None, true, true).show.is_none());
    }
}
