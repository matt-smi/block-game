use avian3d::PhysicsPlugins;
use avian3d::prelude::PhysicsDebugPlugin;
use bevy::prelude::*;
use game::common::{GameState, InputPlugin};
use game::physics::ChunkColliderPlugin;
use game::plugins::*;
use game::ui::UiPlugin;
use game::world::{ChunkHandlerPlugin, WorldPlugin};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Voxel Game".into(),
                name: Some("game.app".into()),
                present_mode: bevy::window::PresentMode::Immediate,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(PhysicsPlugins::default().build())
        .add_plugins(InputPlugin)
        .add_plugins(DebugPlugin {
            should_print: false,
        })
        .init_state::<GameState>()
        .add_plugins(WorldPlugin)
        .add_plugins(UiPlugin)
        .add_plugins(PlayerPlugin)
        //.add_plugins(PhysicsDebugPlugin) //uncomment for collider visuals
        .add_plugins(CameraPlugin)
        .add_plugins(ChunkHandlerPlugin)
        .add_plugins(ChunkColliderPlugin)
        .run();
    println!("Program finished running.");
}
