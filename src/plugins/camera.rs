use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};

use crate::common::GameState;
use crate::plugins::movement::update_position;
use crate::plugins::player::Player;

const ORBIT_DISTANCE: f32 = 10.0;

#[derive(Component)]
pub struct Angles2D {
    pub yaw: f32,
    pub pitch: f32,
}

#[derive(Component)]
pub struct CameraPlugin;
impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(OnEnter(GameState::Playing), lock_cursor)
            .add_systems(OnExit(GameState::Playing), unlock_cursor)
            .add_systems(
                Update,
                orbit
                    .after(update_position)
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(5.0, 5.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn lock_cursor(mut cursor_options: Query<&mut CursorOptions>) {
    let mut cursor_options = cursor_options.single_mut().unwrap();
    cursor_options.grab_mode = CursorGrabMode::Locked;
    cursor_options.visible = false;
}

fn unlock_cursor(mut cursor_options: Query<&mut CursorOptions>) {
    let mut cursor_options = cursor_options.single_mut().unwrap();
    cursor_options.grab_mode = CursorGrabMode::None;
    cursor_options.visible = true;
}

fn orbit(
    mut camera: Single<&mut Transform, With<Camera>>,
    player_transform: Query<&Transform, (With<Player>, Without<Camera>)>,
) {
    let target = player_transform.single().unwrap();
    camera.translation = target.translation - camera.forward() * ORBIT_DISTANCE;
    camera.rotation = target.rotation;
}
