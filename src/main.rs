mod assets;
mod commentary_stub;
mod config;
mod debug;
mod gameplay;
mod states;
mod ui;
mod web;

use assets::AssetRegistryPlugin;
use bevy::asset::{AssetMetaCheck, AssetPlugin};
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::prelude::*;
use bevy::render::settings::{Backends, RenderCreation, WgpuSettings};
use bevy::render::RenderPlugin;
use bevy_egui::EguiPlugin;
#[cfg(feature = "gaussian_splats")]
use bevy_gaussian_splatting::GaussianSplattingPlugin;
use bevy_rapier2d::prelude::*;
use commentary_stub::CommentaryStubPlugin;
use config::ConfigPlugin;
use debug::DebugOverlayPlugin;
use gameplay::GameplayPlugin;
use states::{GameState, GameStatePlugin};
use ui::GameHudPlugin;
use web::WebSupportPlugin;

fn main() {
    let mut primary_window = Window {
        title: "Mr. Autoauto".to_string(),
        resolution: (1280, 720).into(),
        ..default()
    };

    #[cfg(not(target_arch = "wasm32"))]
    {
        primary_window.mode =
            bevy::window::WindowMode::BorderlessFullscreen(bevy::window::MonitorSelection::Primary);
    }

    #[cfg(target_arch = "wasm32")]
    {
        primary_window.mode = bevy::window::WindowMode::Windowed;
        primary_window.fit_canvas_to_parent = true;
        primary_window.prevent_default_event_handling = true;
        primary_window.canvas = Some("#bevy".to_string());
    }

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                meta_check: AssetMetaCheck::Never,
                ..default()
            })
            .set(RenderPlugin {
                render_creation: RenderCreation::Automatic(capture_friendly_wgpu_settings()),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(primary_window),
                ..default()
            }),
    )
    .add_plugins(EguiPlugin::default())
    .add_plugins(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(1.0))
    .add_plugins(FrameTimeDiagnosticsPlugin::default())
    .add_plugins(ConfigPlugin)
    .add_plugins(AssetRegistryPlugin)
    .add_plugins(WebSupportPlugin)
    .add_plugins(DebugOverlayPlugin)
    .add_plugins(GameHudPlugin)
    .add_plugins(CommentaryStubPlugin)
    .add_plugins(GameplayPlugin)
    .insert_resource(ClearColor(Color::srgb(0.04, 0.05, 0.07)))
    .init_state::<GameState>()
    .add_plugins(GameStatePlugin);

    #[cfg(feature = "gaussian_splats")]
    app.add_plugins(GaussianSplattingPlugin);

    app.run();
}

fn capture_friendly_wgpu_settings() -> WgpuSettings {
    let mut settings = WgpuSettings::default();
    #[cfg(target_arch = "wasm32")]
    {
        // Force browser WebGPU on wasm builds so we don't drop to WebGL2,
        // which lacks required storage-buffer capabilities for splat rendering.
        settings.backends = Some(Backends::BROWSER_WEBGPU);
    }
    #[cfg(target_os = "windows")]
    if std::env::var("WGPU_BACKEND").is_err() {
        // Prefer DX12 by default on Windows unless caller explicitly overrides backend.
        settings.backends = Some(Backends::DX12);
    }
    settings
}
