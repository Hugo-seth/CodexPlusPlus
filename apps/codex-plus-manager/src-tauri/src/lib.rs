pub mod commands;
pub mod install;

use tauri::{
    Manager, WindowEvent,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

pub fn run() {
    let Some(_guard) = acquire_single_instance_guard() else {
        return;
    };
    let show_update = commands::startup_should_show_update();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let url = if show_update {
                "index.html?showUpdate=1"
            } else {
                "index.html"
            };
            let window =
                tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::App(url.into()))
                    .title("Codex++ 管理工具")
                    .inner_size(960.0, 720.0)
                    .build()?;

            let win_for_close = window.clone();
            window.on_window_event(move |event| {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = win_for_close.hide();
                }
            });

            setup_tray(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::backend_version,
            commands::startup_options,
            commands::load_overview,
            commands::launch_codex_plus,
            commands::restart_codex_plus,
            commands::load_settings,
            commands::save_settings,
            commands::sync_providers_now,
            commands::load_ads,
            commands::open_external_url,
            commands::install_entrypoints,
            commands::uninstall_entrypoints,
            commands::repair_shortcuts,
            commands::repair_backend,
            commands::check_update,
            commands::perform_update,
            commands::load_watcher_state,
            commands::install_watcher,
            commands::uninstall_watcher,
            commands::enable_watcher,
            commands::disable_watcher,
            commands::read_latest_logs,
            commands::copy_diagnostics,
            commands::reset_settings,
            commands::relay_status,
            commands::apply_relay_injection,
            commands::apply_pure_api_injection,
            commands::clear_relay_injection
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Codex++ manager");
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let show_item = MenuItem::with_id(app, "tray:show", "显示主窗口", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "tray:quit", "退出管理工具", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

    let mut builder = TrayIconBuilder::with_id("codex-plus-manager")
        .tooltip("Codex++ 管理工具")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "tray:show" => reveal_main_window(app),
            "tray:quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                reveal_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }

    builder.build(app)?;
    Ok(())
}

fn reveal_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn acquire_single_instance_guard() -> Option<std::net::TcpListener> {
    match codex_plus_core::ports::acquire_loopback_port_guard(
        codex_plus_core::ports::MANAGER_GUARD_PORT,
    ) {
        Ok(listener) => Some(listener),
        Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {
            let _ = codex_plus_core::diagnostic_log::append_diagnostic_log(
                "manager.already_running",
                serde_json::json!({
                    "guard_port": codex_plus_core::ports::MANAGER_GUARD_PORT
                }),
            );
            None
        }
        Err(error) => {
            let _ = codex_plus_core::diagnostic_log::append_diagnostic_log(
                "manager.guard_failed",
                serde_json::json!({
                    "guard_port": codex_plus_core::ports::MANAGER_GUARD_PORT,
                    "error": error.to_string()
                }),
            );
            Some(
                std::net::TcpListener::bind(("127.0.0.1", 0))
                    .expect("fallback manager guard should bind"),
            )
        }
    }
}
