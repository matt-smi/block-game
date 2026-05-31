use avian3d::prelude::PhysicsSystems;
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use bevy::window::{CursorGrabMode, CursorOptions};
use leafwing_input_manager::prelude::ActionState;

use crate::common::{GameAction, GameState};
use crate::plugins::movement::Movement;
//use crate::plugins::movement::update_position;
use crate::plugins::player::Player;

const ORBIT_DISTANCE: f32 = 6.5;

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
                PostUpdate,
                (orbit, camera_look)
                    .after(PhysicsSystems::Prepare)
                    .before(TransformSystems::Propagate)
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

fn camera_look(
    single_player: Single<(Movement, &ActionState<GameAction>), With<Player>>,
    camera: Single<&mut Transform, (With<Camera>, Without<Player>)>,
) {
    let ((_transform, _, mut angles), action_state) = single_player.into_inner();
    let mut camera_transform = camera.into_inner();

    let mouse_delta = action_state.axis_pair(&GameAction::Look);

    angles.yaw -= mouse_delta.x * 0.005;
    angles.pitch = (angles.pitch - mouse_delta.y * 0.005)
        .clamp(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);

    let yaw_q = Quat::from_rotation_y(angles.yaw);
    let pitch_q = Quat::from_rotation_x(angles.pitch);
    camera_transform.rotation = yaw_q * pitch_q;
}
