use tauri::{
    menu::{CheckMenuItem, CheckMenuItemBuilder, MenuBuilder, MenuItem, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Runtime,
};

use crate::{
    features::proxy::{self, ProxyEnvironmentService},
    services::settings::{self, AppSettings},
};

const PROXY_TOGGLE_ID: &str = "proxy-toggle";
const OPEN_ID: &str = "open-main";
const QUIT_ID: &str = "quit";

pub struct TrayState<R: Runtime> {
    proxy_toggle: CheckMenuItem<R>,
    open: MenuItem<R>,
    quit: MenuItem<R>,
}

struct TrayLabels {
    proxy: &'static str,
    open: &'static str,
    quit: &'static str,
}

fn labels(language: &str) -> TrayLabels {
    match language {
        "zh-CN" => TrayLabels {
            proxy: "代理环境变量",
            open: "打开主界面",
            quit: "退出 ProxyEnv",
        },
        "ja" => TrayLabels {
            proxy: "プロキシ環境変数",
            open: "メイン画面を開く",
            quit: "ProxyEnv を終了",
        },
        "ko" => TrayLabels {
            proxy: "프록시 환경 변수",
            open: "메인 화면 열기",
            quit: "ProxyEnv 종료",
        },
        _ => TrayLabels {
            proxy: "Proxy environment",
            open: "Open main window",
            quit: "Quit ProxyEnv",
        },
    }
}

pub fn setup<R: Runtime>(app: &AppHandle<R>, settings: &AppSettings) -> tauri::Result<()> {
    let language = settings.clone().resolved_language();
    let labels = labels(language);
    let enabled = ProxyEnvironmentService::status()
        .map(|status| status.enabled)
        .unwrap_or(false);
    let proxy_toggle = CheckMenuItemBuilder::with_id(PROXY_TOGGLE_ID, labels.proxy)
        .checked(enabled)
        .build(app)?;
    let open = MenuItemBuilder::with_id(OPEN_ID, labels.open).build(app)?;
    let quit = MenuItemBuilder::with_id(QUIT_ID, labels.quit).build(app)?;
    let menu = MenuBuilder::new(app)
        .item(&proxy_toggle)
        .separator()
        .item(&open)
        .item(&quit)
        .build()?;

    let mut builder = TrayIconBuilder::with_id("main-tray")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("ProxyEnv")
        .on_menu_event(|app, event| match event.id().as_ref() {
            PROXY_TOGGLE_ID => toggle_proxy(app),
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
    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }
    builder.build(app)?;
    app.manage(TrayState {
        proxy_toggle,
        open,
        quit,
    });
    Ok(())
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
    let result = ProxyEnvironmentService::status().and_then(|status| {
        let settings = settings::load()?;
        if status.enabled {
            ProxyEnvironmentService::disable()
        } else {
            let candidate = proxy::detect()?
                .into_iter()
                .find(|candidate| candidate.listening);
            if let Some(candidate) = candidate {
                ProxyEnvironmentService::enable_for_proxy(
                    &candidate.host,
                    candidate.port,
                    candidate.protocol,
                    &settings.proxy_variables,
                )
            } else {
                ProxyEnvironmentService::enable(&settings.proxy_variables)
            }
        }
    });
    match result {
        Ok(status) => {
            update_proxy_state(app, status.enabled);
            let _ = app.emit("proxy-state-changed", status);
        }
        Err(error) => {
            let _ = app.emit("operation-error", error.to_string());
            if let Ok(status) = ProxyEnvironmentService::status() {
                update_proxy_state(app, status.enabled);
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
        let labels = labels(language);
        let _ = state.proxy_toggle.set_text(labels.proxy);
        let _ = state.open.set_text(labels.open);
        let _ = state.quit.set_text(labels.quit);
    }
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
