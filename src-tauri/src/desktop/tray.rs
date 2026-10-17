use tauri::{
    menu::{CheckMenuItem, CheckMenuItemBuilder, MenuBuilder, MenuItem, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Runtime,
};

use crate::{
    features::{
        proxy::{active, ProxyEnvironmentService},
        remote_bridge::tray_state::{self, Connection, Snapshot},
    },
    services::settings::{self, AppSettings},
};
use std::{sync::Mutex, time::Duration};

const PROXY_TOGGLE_ID: &str = "proxy-toggle";
const LOCAL_ID: &str = "open-local";
const REMOTE_ID: &str = "open-remote";
const NETWORK_ID: &str = "bridge-network";
const AI_ID: &str = "bridge-ai";
const OPEN_ID: &str = "open-main";
const QUIT_ID: &str = "quit";

pub struct TrayState<R: Runtime> {
    proxy_toggle: CheckMenuItem<R>,
    local: MenuItem<R>,
    remote_status: MenuItem<R>,
    remote: MenuItem<R>,
    network: MenuItem<R>,
    ai: MenuItem<R>,
    open: MenuItem<R>,
    quit: MenuItem<R>,
    language: Mutex<String>,
    // Native-render deduplication only. Every action re-reads backend state.
    rendered: Mutex<Option<Snapshot>>,
}

struct TrayLabels {
    proxy: &'static str,
    local: &'static str,
    remote: &'static str,
    bridge: &'static str,
    network: &'static str,
    ai: &'static str,
    states: [&'static str; 5],
    on: &'static str,
    off: &'static str,
    open: &'static str,
    quit: &'static str,
}

fn labels(language: &str) -> TrayLabels {
    match language {
        "zh-CN" => TrayLabels {
            proxy: "本机代理环境变量",
            local: "打开本机环境",
            remote: "打开远程桥接",
            bridge: "远程桥接",
            network: "桥接本机网络",
            ai: "CC Switch AI 路由",
            states: ["未连接", "连接中", "已连接", "重连中", "需处理"],
            on: "开启",
            off: "关闭",
            open: "打开主界面",
            quit: "退出 RouteSpan",
        },
        "ja" => TrayLabels {
            proxy: "ローカルプロキシ環境変数",
            local: "ローカル環境を開く",
            remote: "リモートブリッジを開く",
            bridge: "リモートブリッジ",
            network: "ローカルネットワークをブリッジ",
            ai: "CC Switch AI ルート",
            states: ["未接続", "接続中", "接続済み", "再接続中", "要対応"],
            on: "有効",
            off: "無効",
            open: "メイン画面を開く",
            quit: "RouteSpan を終了",
        },
        "ko" => TrayLabels {
            proxy: "로컬 프록시 환경 변수",
            local: "로컬 환경 열기",
            remote: "원격 브리지 열기",
            bridge: "원격 브리지",
            network: "로컬 네트워크 브리지",
            ai: "CC Switch AI 라우팅",
            states: ["연결 안 됨", "연결 중", "연결됨", "재연결 중", "조치 필요"],
            on: "켜짐",
            off: "꺼짐",
            open: "메인 화면 열기",
            quit: "RouteSpan 종료",
        },
        _ => TrayLabels {
            proxy: "Local proxy environment variables",
            local: "Open local environment",
            remote: "Open remote bridge",
            bridge: "Remote bridge",
            network: "Bridge local network",
            ai: "CC Switch AI route",
            states: [
                "Disconnected",
                "Connecting",
                "Connected",
                "Reconnecting",
                "Needs attention",
            ],
            on: "On",
            off: "Off",
            open: "Open main window",
            quit: "Quit RouteSpan",
        },
    }
}

impl TrayLabels {
    fn status(&self, connection: Connection) -> String {
        let index = match connection {
            Connection::Disconnected => 0,
            Connection::Connecting => 1,
            Connection::Connected => 2,
            Connection::Reconnecting => 3,
            Connection::Attention => 4,
        };
        format!("{} · {}", self.bridge, self.states[index])
    }

    fn capability(&self, title: &str, enabled: bool) -> String {
        let state = if enabled { self.on } else { self.off };
        format!("{title} · {state}")
    }
}

#[derive(Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
enum TrayNavigation {
    Local,
    Remote,
    Proxy,
    Cc,
}

fn navigate<R: Runtime>(app: &AppHandle<R>, action: TrayNavigation) {
    show_main_window(app);
    let action = match action {
        TrayNavigation::Proxy | TrayNavigation::Cc if !tray_state::can_change() => {
            TrayNavigation::Remote
        }
        action => action,
    };
    // Only a fixed navigation intent crosses IPC; mutations belong to the reviewed UI flow.
    let _ = app.emit("tray-navigation", action);
}

pub fn setup<R: Runtime>(app: &AppHandle<R>, settings: &AppSettings) -> tauri::Result<()> {
    let language = settings.clone().resolved_language();
    let labels = labels(language);
    let enabled = active::status(&settings.proxy_variables)
        .map(|status| status.state.is_configured())
        .unwrap_or(false);
    let proxy_toggle = CheckMenuItemBuilder::with_id(PROXY_TOGGLE_ID, labels.proxy)
        .checked(enabled)
        .build(app)?;
    // Subscribe before reading so a transition during setup cannot be missed.
    let changes = tray_state::subscribe();
    let snapshot = tray_state::snapshot();
    let title = MenuItemBuilder::with_id("tray-title", "RouteSpan")
        .enabled(false)
        .build(app)?;
    let local = MenuItemBuilder::with_id(LOCAL_ID, labels.local).build(app)?;
    let remote_status =
        MenuItemBuilder::with_id("bridge-status", labels.status(snapshot.connection))
            .enabled(false)
            .build(app)?;
    let remote = MenuItemBuilder::with_id(REMOTE_ID, labels.remote).build(app)?;
    let network = MenuItemBuilder::with_id(
        NETWORK_ID,
        labels.capability(labels.network, snapshot.proxy),
    )
    .enabled(snapshot.can_change)
    .build(app)?;
    let ai = MenuItemBuilder::with_id(AI_ID, labels.capability(labels.ai, snapshot.cc))
        .enabled(snapshot.can_change)
        .build(app)?;
    let open = MenuItemBuilder::with_id(OPEN_ID, labels.open).build(app)?;
    let quit = MenuItemBuilder::with_id(QUIT_ID, labels.quit).build(app)?;
    let menu = MenuBuilder::new(app)
        .item(&title)
        .separator()
        .item(&proxy_toggle)
        .item(&local)
        .separator()
        .item(&remote_status)
        .item(&remote)
        .item(&network)
        .item(&ai)
        .separator()
        .item(&open)
        .item(&quit)
        .build()?;

    let builder = TrayIconBuilder::with_id("main-tray")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("RouteSpan")
        .icon(tauri::include_image!("icons/tray.png"))
        .on_menu_event(|app, event| match event.id().as_ref() {
            PROXY_TOGGLE_ID => toggle_proxy(app),
            LOCAL_ID => navigate(app, TrayNavigation::Local),
            REMOTE_ID => navigate(app, TrayNavigation::Remote),
            NETWORK_ID => navigate(app, TrayNavigation::Proxy),
            AI_ID => navigate(app, TrayNavigation::Cc),
            OPEN_ID => show_main_window(app),
            QUIT_ID => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                show_main_window(tray.app_handle());
            }
        });
    builder.build(app)?;
    app.manage(TrayState {
        proxy_toggle,
        local,
        remote_status,
        remote,
        network,
        ai,
        open,
        quit,
        language: Mutex::new(language.to_owned()),
        rendered: Mutex::new(Some(snapshot)),
    });
    let app = app.clone();
    std::thread::spawn(move || {
        while let Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) =
            changes.recv_timeout(Duration::from_secs(5))
        {
            // Changes notify immediately. Timeout only retries native menu failures;
            // both paths read memory, never trigger detection or lock bridge Store.
            if !refresh_remote_menu(&app) {
                break;
            }
        }
    });
    Ok(())
}

fn refresh_remote_menu<R: Runtime>(app: &AppHandle<R>) -> bool {
    let handle = app.clone();
    app.run_on_main_thread(move || {
        // Read at render time, not queue time, to avoid replaying an older state.
        let snapshot = tray_state::snapshot();
        let Some(state) = handle.try_state::<TrayState<R>>() else {
            return;
        };
        let (Ok(language), Ok(mut rendered)) = (state.language.lock(), state.rendered.lock())
        else {
            return;
        };
        if *rendered == Some(snapshot) {
            return;
        }
        let labels = labels(&language);
        let result = (|| -> tauri::Result<()> {
            state.proxy_toggle.set_text(labels.proxy)?;
            state.local.set_text(labels.local)?;
            state.remote.set_text(labels.remote)?;
            state.open.set_text(labels.open)?;
            state.quit.set_text(labels.quit)?;
            state
                .remote_status
                .set_text(labels.status(snapshot.connection))?;
            state
                .network
                .set_text(labels.capability(labels.network, snapshot.proxy))?;
            state
                .ai
                .set_text(labels.capability(labels.ai, snapshot.cc))?;
            state.network.set_enabled(snapshot.can_change)?;
            state.ai.set_enabled(snapshot.can_change)?;
            Ok(())
        })();
        // A failed native update is retried next wakeup, not cached as success.
        *rendered = result.ok().map(|()| snapshot);
    })
    .is_ok()
}

pub fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn hide_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

fn toggle_proxy<R: Runtime>(app: &AppHandle<R>) {
    let result = settings::load().and_then(|settings| {
        let status = active::status(&settings.proxy_variables)?;
        if status.state.is_configured() {
            ProxyEnvironmentService::disable()?;
        } else {
            active::with_current(Some(status.active_proxy.revision), |candidate| {
                ProxyEnvironmentService::sync(
                    &candidate.host,
                    candidate.port,
                    candidate.protocol,
                    &settings.proxy_variables,
                )
            })?;
        }
        active::status(&settings.proxy_variables)
    });
    match result {
        Ok(status) => {
            update_proxy_state(app, status.state.is_configured());
            let _ = app.emit("proxy-state-changed", status);
        }
        Err(error) => {
            let _ = app.emit("operation-error", error.to_string());
            if let Ok(settings) = settings::load() {
                if let Ok(status) = active::status(&settings.proxy_variables) {
                    update_proxy_state(app, status.state.is_configured());
                }
            }
        }
    }
}

pub fn update_proxy_state<R: Runtime>(app: &AppHandle<R>, enabled: bool) {
    if let Some(state) = app.try_state::<TrayState<R>>() {
        let _ = state.proxy_toggle.set_checked(enabled);
    }
}

pub fn update_language<R: Runtime>(app: &AppHandle<R>, language: &str) {
    if let Some(state) = app.try_state::<TrayState<R>>() {
        if let (Ok(mut current), Ok(mut rendered)) = (state.language.lock(), state.rendered.lock())
        {
            *current = language.to_owned();
            *rendered = None;
        }
    }
    refresh_remote_menu(app);
}

pub fn should_hide_on_close() -> bool {
    settings::load()
        .map(|settings| settings.close_to_tray)
        .unwrap_or(true)
}

pub fn should_start_silent() -> bool {
    std::env::args().any(|arg| arg == "--autostart")
        && settings::load()
            .map(|settings| settings.silent_start)
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_locale_describes_transport_and_each_capability_in_text() {
        for locale in ["zh-CN", "en", "ja", "ko"] {
            let copy = labels(locale);
            for connection in [
                Connection::Disconnected,
                Connection::Connecting,
                Connection::Connected,
                Connection::Reconnecting,
                Connection::Attention,
            ] {
                assert!(copy.status(connection).starts_with(copy.bridge));
            }
            assert_ne!(
                copy.capability(copy.network, true),
                copy.capability(copy.network, false)
            );
            assert_ne!(
                copy.capability(copy.ai, true),
                copy.capability(copy.ai, false)
            );
            for text in [
                copy.proxy,
                copy.local,
                copy.remote,
                copy.network,
                copy.ai,
                copy.open,
                copy.quit,
            ] {
                assert!(!text.is_empty());
                assert!(!text.contains("127.0.0.1"));
                assert!(!text.contains("Token"));
            }
        }
        assert_eq!(labels("system").open, labels("en").open);
    }

    #[test]
    fn navigation_payload_is_an_allowlisted_intent_not_a_target_or_command() {
        for (action, expected) in [
            (TrayNavigation::Local, "local"),
            (TrayNavigation::Remote, "remote"),
            (TrayNavigation::Proxy, "proxy"),
            (TrayNavigation::Cc, "cc"),
        ] {
            assert_eq!(serde_json::to_value(action).unwrap(), expected);
        }
    }
}
