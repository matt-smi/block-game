use bevy::prelude::*;
use game::common::{GameState, InputPlugin};
use game::plugins::*;
use game::ui::UiPlugin;
use game::world::{ChunkHandlerPlugin, WorldPlugin};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Voxel Game".into(),
                name: Some("game.app".into()),
                present_mode: bevy::window::PresentMode::AutoNoVsync,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(InputPlugin)
        .add_plugins(DebugPlugin {
            should_print: false,
        })
        .init_state::<GameState>()
        .add_plugins(WorldPlugin)
        .add_plugins(UiPlugin)
        .add_plugins(PlayerPlugin)
        .add_plugins(MovementPlugin)
        .add_plugins(CameraPlugin)
        .add_plugins(ChunkHandlerPlugin)
        .run();
    println!("Program finished running.");
}
