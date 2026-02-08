mod assets;
mod commentary_stub;
mod config;
mod debug;
mod gameplay;
mod states;
mod ui;

use assets::AssetRegistryPlugin;
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

fn main() {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(RenderPlugin {
                render_creation: RenderCreation::Automatic(capture_friendly_wgpu_settings()),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Mr. Autoauto".to_string(),
                    resolution: (1280, 720).into(),
                    mode: bevy::window::WindowMode::BorderlessFullscreen(
                        bevy::window::MonitorSelection::Primary,
                    ),
                    ..default()
                }),
                ..default()
            }),
    )
    .add_plugins(EguiPlugin::default())
    .add_plugins(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(1.0))
    .add_plugins(FrameTimeDiagnosticsPlugin::default())
    .add_plugins(ConfigPlugin)
    .add_plugins(AssetRegistryPlugin)
    .add_plugins(DebugOverlayPlugin)
    .add_plugins(GameHudPlugin)
    .add_plugins(CommentaryStubPlugin)
    .add_plugins(GameplayPlugin)
    .init_state::<GameState>()
    .add_plugins(GameStatePlugin);

    #[cfg(feature = "gaussian_splats")]
    app.add_plugins(GaussianSplattingPlugin);

    app.run();
}

fn capture_friendly_wgpu_settings() -> WgpuSettings {
    let mut settings = WgpuSettings::default();
    #[cfg(target_os = "windows")]
    if std::env::var("WGPU_BACKEND").is_err() {
        // Prefer DX12 by default on Windows unless caller explicitly overrides backend.
        settings.backends = Some(Backends::DX12);
    }
    settings
}
