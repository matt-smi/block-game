use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use crate::common::*;
use crate::plugins::movement::Velocity;
use crate::world::VoxelResource;
use crate::world::init_resources;

const INIT_VELOCITY: Vec3 = Vec3::ZERO;
const PLAYER_SPEED: f32 = 15.0;
const PLAYER_SCALE: f32 = 0.5;

type Movement = (
    &'static mut Transform,
    &'static mut Velocity,
    &'static mut Angles2D,
);

#[derive(Component)]
pub struct Player;

#[derive(Component)]
struct Angles2D {
    yaw: f32,
    pitch: f32,
}

pub struct PlayerPlugin;
impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_player.after(init_resources))
            .add_systems(
                Update,
                (player_look, player_move).run_if(in_state(GameState::Playing)),
            );
    }
}

fn spawn_player(mut commands: Commands, voxel: Res<VoxelResource>) {
    commands.spawn((
        Mesh3d(voxel.mesh.clone()),
        MeshMaterial3d(voxel.materials[0].clone()),
        Transform {
            scale: Vec3::new(PLAYER_SCALE, PLAYER_SCALE, PLAYER_SCALE),
            ..default()
        },
        Angles2D {
            yaw: 0.0,
            pitch: 0.0,
        },
        Player,
        Velocity {
            value: INIT_VELOCITY,
        },
        default_game_action_map(),
    ));
}

fn player_look(single: Single<(Movement, &ActionState<GameAction>), With<Player>>) {
    let ((mut transform, _, mut angles), action_state) = single.into_inner();

    let mouse_delta = action_state.axis_pair(&GameAction::Look);
    angles.yaw -= mouse_delta.x * MOUSE_SENSITIVITY;
    angles.pitch = (angles.pitch - mouse_delta.y * MOUSE_SENSITIVITY)
        .clamp(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);

    transform.rotation = Quat::from_rotation_y(angles.yaw) * Quat::from_rotation_x(angles.pitch);
}

fn player_move(single: Single<(Movement, &ActionState<GameAction>), With<Player>>) {
    let ((_, mut velocity, angles), action_state) = single.into_inner();
    let mut direction = Vec3::ZERO;
    let yaw_rot = Quat::from_rotation_y(angles.yaw);

    // Horizontal direction handling
    let hori = action_state.clamped_axis_pair(&GameAction::MoveHorizontal);
    direction += hori.x * (yaw_rot * Vec3::X).normalize();
    direction += hori.y * -(yaw_rot * Vec3::Z).normalize();

    // Vertical direction handling
    let vert = action_state.clamped_value(&GameAction::MoveVertical);
    direction += vert * Vec3::Y;

    velocity.value = direction.normalize_or_zero() * PLAYER_SPEED;
}
