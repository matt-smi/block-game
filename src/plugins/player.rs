use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::time::Fixed;
use leafwing_input_manager::prelude::*;

use crate::common::*;
use crate::plugins::camera::Angles2D;
use crate::plugins::movement::*;
use crate::world::{CHUNK_DIMENSION, ChunkVoxels, global_voxel_to_chunk, world_to_global_voxel};

const PLAYER_SPEED: f32 = 60.0;
const JUMP_VELOCITY: f32 = 15.5;
const PLAYER_GRAVITY: f32 = 35.0;
const PLAYER_SCALE: f32 = 0.5;
const PLAYER_SPRINT_SPEED: f32 = PLAYER_SPEED * 1.5;
const PLAYER_HALF_EXTENTS: Vec3 = Vec3::new(0.48, 1.0, 0.48);
const PLAYER_UPWARD_HALF_EXTENTS: Vec3 = Vec3::new(0.36, 1.0, 0.36);
const PLAYER_SKIN_WIDTH: f32 = 0.001;
const COLLISION_BINARY_STEPS: usize = 10;
const COYOTE_TIME: f32 = 0.08;
const BLOCK_TOLERANCE: f32 = 0.001;
const AXIS_X: usize = 0;
const AXIS_Y: usize = 1;
const AXIS_Z: usize = 2;

#[derive(Component)]
pub struct Player;

#[derive(Component, Default)]
struct PlayerMotionState {
    coyote_time_remaining: f32,
    grounded: bool,
}

pub struct PlayerPlugin;
impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_player)
            .add_systems(Update, player_look.run_if(in_state(GameState::Playing)))
            .add_systems(
                FixedUpdate,
                player_move.run_if(in_state(GameState::Playing)),
            );
    }
}

fn spawn_player(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let body = meshes.add(Cuboid::new(
        4. * PLAYER_SCALE,
        8. * PLAYER_SCALE,
        4. * PLAYER_SCALE,
    ));

    let handle = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        ..default()
    });

    commands.spawn((
        Mesh3d(body),
        MeshMaterial3d(handle),
        Transform {
            scale: Vec3::new(PLAYER_SCALE, PLAYER_SCALE, PLAYER_SCALE),
            translation: Vec3::new(0., 49., 0.),
            ..default()
        },
        Angles2D {
            yaw: 0.0,
            pitch: 0.0,
        },
        PlayerMotionState::default(),
        Player,
        LinearVelocity::default(),
        default_game_action_map(),
    ));
}

fn player_look(single: Single<(Movement, &ActionState<GameAction>), With<Player>>) {
    let ((mut transform, _, mut angles), action_state) = single.into_inner();

    let mouse_delta = action_state.axis_pair(&GameAction::Look);
    angles.yaw -= mouse_delta.x * MOUSE_SENSITIVITY;
    angles.pitch = (angles.pitch - mouse_delta.y * MOUSE_SENSITIVITY)
        .clamp(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);

    transform.rotation = Quat::from_rotation_y(angles.yaw) //* Quat::from_rotation_x(angles.pitch);
}

fn player_move(
    single: Single<(Movement, &ActionState<GameAction>, &mut PlayerMotionState), With<Player>>,
    chunk_voxels: Res<ChunkVoxels>,
    time: Res<Time<Fixed>>,
) {
    let ((mut transform, mut linear_velocity, angles), action_state, mut motion_state) =
        single.into_inner();
    let mut direction = Vec3::ZERO;
    let yaw_rot = Quat::from_rotation_y(angles.yaw);
    let dt = time.delta_secs();

    let hori = action_state.clamped_axis_pair(&GameAction::MoveHorizontal);
    direction += hori.x * (yaw_rot * Vec3::X).normalize();
    direction += hori.y * -(yaw_rot * Vec3::Z).normalize();

    let mut horizontal_velocity = direction.normalize_or_zero();

    if action_state.pressed(&GameAction::Sprint) {
        horizontal_velocity *= PLAYER_SPRINT_SPEED;
    } else {
        horizontal_velocity *= PLAYER_SPEED;
    }

    linear_velocity.x = horizontal_velocity.x;
    linear_velocity.z = horizontal_velocity.z;

    if motion_state.grounded {
        motion_state.coyote_time_remaining = COYOTE_TIME;
    } else {
        motion_state.coyote_time_remaining = (motion_state.coyote_time_remaining - dt).max(0.0);
    }

    let mut jumped_this_frame = false;
    if ((motion_state.coyote_time_remaining > 0.0) || motion_state.grounded)
        && action_state.just_pressed(&GameAction::Jump)
    {
        linear_velocity.y = JUMP_VELOCITY;
        motion_state.coyote_time_remaining = 0.0;
        motion_state.grounded = false;
        jumped_this_frame = true;
    }
    if !jumped_this_frame {
        linear_velocity.y -= PLAYER_GRAVITY * dt;
    }

    let intended_y_delta = linear_velocity.y * dt;
    let mut next_position = transform.translation;

    if intended_y_delta > 0.0 {
        next_position = move_axis(
            next_position,
            AXIS_Y,
            intended_y_delta,
            &chunk_voxels,
            PLAYER_UPWARD_HALF_EXTENTS,
        );
        next_position = move_axis(
            next_position,
            AXIS_X,
            linear_velocity.x * dt,
            &chunk_voxels,
            PLAYER_HALF_EXTENTS,
        );
        next_position = move_axis(
            next_position,
            AXIS_Z,
            linear_velocity.z * dt,
            &chunk_voxels,
            PLAYER_HALF_EXTENTS,
        );
    } else {
        next_position = move_axis(
            next_position,
            AXIS_X,
            linear_velocity.x * dt,
            &chunk_voxels,
            PLAYER_HALF_EXTENTS,
        );
        next_position = move_axis(
            next_position,
            AXIS_Z,
            linear_velocity.z * dt,
            &chunk_voxels,
            PLAYER_HALF_EXTENTS,
        );
        next_position = move_axis(
            next_position,
            AXIS_Y,
            intended_y_delta,
            &chunk_voxels,
            PLAYER_HALF_EXTENTS,
        );
    }

    let actual_y_delta = next_position.y - transform.translation.y;
    let y_shortfall = intended_y_delta.abs() - actual_y_delta.abs();
    let y_blocked = y_shortfall > BLOCK_TOLERANCE;
    if y_blocked {
        linear_velocity.y = 0.0;
    }
    motion_state.grounded = y_blocked && intended_y_delta < 0.0;

    transform.translation = next_position;
}

fn move_axis(
    position: Vec3,
    axis: usize,
    delta: f32,
    chunk_voxels: &ChunkVoxels,
    half_extents: Vec3,
) -> Vec3 {
    if delta.abs() <= f32::EPSILON {
        return position;
    }

    let mut target = position;
    target[axis] += delta;
    if !collides_at(target, chunk_voxels, half_extents) {
        return target;
    }

    let mut low = 0.0;
    let mut high = 1.0;
    for _ in 0..COLLISION_BINARY_STEPS {
        let mid = (low + high) * 0.5;
        let mut test_position = position;
        test_position[axis] += delta * mid;
        if collides_at(test_position, chunk_voxels, half_extents) {
            high = mid;
        } else {
            low = mid;
        }
    }

    let mut resolved = position;
    resolved[axis] += delta * low;
    resolved
}

fn collides_at(position: Vec3, chunk_voxels: &ChunkVoxels, half_extents: Vec3) -> bool {
    let min = position - half_extents + Vec3::splat(PLAYER_SKIN_WIDTH);
    let max = position + half_extents - Vec3::splat(PLAYER_SKIN_WIDTH);

    let min_voxel = world_to_global_voxel(min);
    let max_voxel = world_to_global_voxel(max);
    let (min_chunk, _) = global_voxel_to_chunk(min_voxel);
    let (max_chunk, _) = global_voxel_to_chunk(max_voxel);

    for cx in min_chunk.x..=max_chunk.x {
        for cy in min_chunk.y..=max_chunk.y {
            for cz in min_chunk.z..=max_chunk.z {
                let chunk_pos = IVec3::new(cx, cy, cz);
                let Some(voxel_data) = chunk_voxels.chunks.get(&chunk_pos) else {
                    continue;
                };

                let chunk_min_x = (cx * CHUNK_DIMENSION as i32).max(min_voxel.x);
                let chunk_max_x =
                    (cx * CHUNK_DIMENSION as i32 + CHUNK_DIMENSION as i32 - 1).min(max_voxel.x);
                let chunk_min_y = (cy * CHUNK_DIMENSION as i32).max(min_voxel.y);
                let chunk_max_y =
                    (cy * CHUNK_DIMENSION as i32 + CHUNK_DIMENSION as i32 - 1).min(max_voxel.y);
                let chunk_min_z = (cz * CHUNK_DIMENSION as i32).max(min_voxel.z);
                let chunk_max_z =
                    (cz * CHUNK_DIMENSION as i32 + CHUNK_DIMENSION as i32 - 1).min(max_voxel.z);

                for x in chunk_min_x..=chunk_max_x {
                    let lx = x.rem_euclid(CHUNK_DIMENSION as i32) as u32;
                    for y in chunk_min_y..=chunk_max_y {
                        let ly = y.rem_euclid(CHUNK_DIMENSION as i32) as u32;
                        for z in chunk_min_z..=chunk_max_z {
                            let lz = z.rem_euclid(CHUNK_DIMENSION as i32) as u32;
                            if voxel_data.is_solid(lx, ly, lz) {
                                return true;
                            }
                        }
                    }
                }
            }
        }
    }

    false
}
