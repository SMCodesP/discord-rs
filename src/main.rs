extern crate gpui_kit as gpui;

use std::sync::Arc;
use discord_rs::app::RootView;
use discord_rs::protocol::ReqwestHttpClient;
use gpui::*;

fn main() {
    env_logger::init();

    // Install default crypto provider for rustls 0.23 (TLS connections on Linux)
    let _ = rustls::crypto::ring::default_provider().install_default();

    // Initialize multi-threaded Tokio runtime for async Discord Gateway WebSocket & REST requests
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Falha ao inicializar o Tokio Runtime");
    let _guard = rt.enter();

    // Inicializar o Serviço Nativo de Telemetria e Monitoramento de Paridade
    let (_telemetry_service, telemetry_handle) = discord_rs::telemetry::TelemetryService::start(
        discord_rs::telemetry::TelemetryConfig::default(),
    );

    platform::application()
        .with_http_client(Arc::new(ReqwestHttpClient::new()))
        .with_assets(discord_rs::tokens::icons::AppAssetSource)
        .run(move |cx: &mut App| {
            // Inicializa o gpui-kit (tema, Root, overlays, keybindings) antes de abrir a janela.
            gpui::init(cx);
            // Tema escuro Discord registrado pelo caminho oficial do kit (KTheme).
            // A troca Dark/Light/Darker/Midnight e as cores customizadas da conta
            // são reaplicadas em tempo de execução por `kit_theme::apply_theme`.
            discord_rs::tokens::kit_theme::install(cx);

            // Registra no log a família que o sistema realmente resolveu. Isso
            // mantém a fonte efetiva auditável sem alegar paridade de glifos com
            // a `gg sans`, que não é distribuída por este projeto.
            let text_system = cx.text_system();
            let requested = gpui::font(discord_rs::tokens::FONT_FAMILY_PRIMARY);
            let font_id = text_system.resolve_font(&requested);
            let effective_font = text_system
                .get_font_for_id(font_id)
                .map(|font| font.family.to_string())
                .unwrap_or_else(|| "<fallback do sistema>".to_string());
            let font_size = discord_rs::tokens::SIZE_BASE;
            log::info!(
                "Tipografia: solicitada='{}' efetiva='{}' tamanho={}px line-height={}px (fallback aberto; sem paridade de glifos com gg sans)",
                discord_rs::tokens::FONT_FAMILY_PRIMARY,
                effective_font,
                font_size,
                discord_rs::tokens::LH_BASE
            );

            // Dynamically limit window bounds within primary display bounds to avoid Wayland compositor buffer crashes
            let (target_w, target_h) = if let Some(display) = cx.primary_display() {
                let screen = display.bounds();
                let screen_w = screen.size.width / px(1.0);
                let screen_h = screen.size.height / px(1.0);
                let w = (screen_w * 0.85).clamp(940.0, 1280.0);
                let h = (screen_h * 0.85).clamp(600.0, 800.0);
                (w, h)
            } else {
                (1280.0, 800.0)
            };

            let window_bounds = Bounds::centered(None, size(px(target_w), px(target_h)), cx);

            let window_options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(window_bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Discord".into()),
                    appears_transparent: false,
                    traffic_light_position: None,
                }),
                focus: true,
                show: true,
                kind: WindowKind::Normal,
                is_movable: true,
                is_resizable: true,
                ..Default::default()
            };

            let tel = telemetry_handle.clone();
            gpui::open_window(window_options, cx, move |_window, cx| {
                let tel = tel.clone();
                cx.new(move |cx| RootView::new(tel, cx))
            })
            .expect("Falha ao abrir a janela do Discord GPUI");
        });
}
