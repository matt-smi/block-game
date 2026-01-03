use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::transform::TransformSystem;
use bevy::window::CursorGrabMode;

use crate::common::GameState;
use crate::common::*;
use crate::plugins::player::Player;
use crate::plugins::player::*;
use leafwing_input_manager::prelude::*;

const MAX_ORBIT_DISTANCE: f32 = 10.0;

#[derive(Component)]
pub struct OrbitDistance {
    distance: f32,
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
                orbit
                    .before(TransformSystem::TransformPropagate)
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(Update, orbit_range.run_if(in_state(GameState::Playing)))
            .add_systems(
                PostUpdate,
                camera_look
                    .before(TransformSystem::TransformPropagate)
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(5.0, 5.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        OrbitDistance {
            distance: MAX_ORBIT_DISTANCE,
        },
    ));
}

fn lock_cursor(mut windows: Query<&mut Window>) {
    let mut window = windows.single_mut().unwrap();
    window.cursor_options.grab_mode = CursorGrabMode::Locked;
    window.cursor_options.visible = false;
}

fn unlock_cursor(mut windows: Query<&mut Window>) {
    let mut window = windows.single_mut().unwrap();
    window.cursor_options.grab_mode = CursorGrabMode::None;
    window.cursor_options.visible = true;
}

fn orbit(
    mut camera: Query<(&mut Transform, &OrbitDistance), With<Camera>>,
    player_transform: Query<&Transform, (With<Player>, Without<Camera>)>,
) {
    let (mut camera_transform, orbit_distance) = camera.single_mut().unwrap();
    let target = player_transform.single().unwrap();
    camera_transform.translation =
        target.translation - camera_transform.forward() * orbit_distance.distance;
}

fn camera_look(
    single_player: Single<(Movement, &ActionState<GameAction>), With<Player>>,
    camera: Single<&mut Transform, (With<Camera>, Without<Player>)>,
) {
    let ((_transform, _, mut angles), action_state) = single_player.into_inner();
    let mut camera_transform = camera.into_inner();

    let mouse_delta = action_state.axis_pair(&GameAction::Look);

    angles.yaw -= mouse_delta.x * MOUSE_SENSITIVITY;
    angles.pitch = (angles.pitch - mouse_delta.y * MOUSE_SENSITIVITY)
        .clamp(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);

    let yaw_q = Quat::from_rotation_y(angles.yaw);
    let pitch_q = Quat::from_rotation_x(angles.pitch);
    camera_transform.rotation = yaw_q * pitch_q;
}

fn orbit_range(
    mut evr_scroll: EventReader<MouseWheel>,
    camera: Single<&mut OrbitDistance, With<Camera>>,
) {
    let mut orbit_distance = camera.into_inner();
    for ev in evr_scroll.read() {
        match ev.unit {
            MouseScrollUnit::Line => {
                if ev.y < 0.0 {
                    orbit_distance.distance =
                        f32::min(orbit_distance.distance + 1.0, MAX_ORBIT_DISTANCE);
                } else {
                    orbit_distance.distance =
                        f32::max(orbit_distance.distance - 1.0, MAX_ORBIT_DISTANCE / 3.);
                }
            }
            MouseScrollUnit::Pixel => {
                //println!("Scroll (pixel units): vertical: {}, horizontal: {}", ev.y, ev.x);
            }
        }
    }
}
