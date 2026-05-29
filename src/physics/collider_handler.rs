use avian3d::prelude::{Collider, CollisionLayers, CollisionMargin};
use bevy::prelude::*;

use crate::physics::{COLLIDER_PRUNE_DISTANCE, Layers};
use crate::plugins::player::Player;
use crate::world::{ChunkEntities, get_chunk_index};

pub struct ChunkColliderPlugin;

impl Plugin for ChunkColliderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, (generate_colliders, prune_colliders));
    }
}

// TODO: change With to be more generic, e.g. LinearVelocity doesn't work since chunks have it???
fn generate_colliders(
    moving_objects: Query<&Transform, With<Player>>,
    chunk_entities: Res<ChunkEntities>,
    mesh_query: Query<&Mesh3d, Without<Collider>>,
    meshes: Res<Assets<Mesh>>,
    mut commands: Commands,
) {
    for transform in &moving_objects {
        let chunk_pos = get_chunk_index(transform.translation);
        if let Some(entities) = chunk_entities.chunks.get(&chunk_pos) {
            for &entity_id in entities {
                if let Ok(mesh3d) = mesh_query.get(entity_id)
                    && let Some(mesh) = meshes.get(&mesh3d.0)
                {
                    let collider = Collider::trimesh_from_mesh(mesh);
                    if let Some(collider) = collider {
                        commands.entity(entity_id).insert((
                            collider,
                            CollisionLayers::new([Layers::Terrain], [Layers::Player]),
                            CollisionMargin(0.005),
                        ));
                    }
                }
            }
        }
    }
}

// probably want to combine this with chunk render pruning logic...
fn prune_colliders(
    colliders_transform: Query<(Entity, &Transform), With<Collider>>,
    player_transform: Query<&Transform, With<Player>>,
    mut commands: Commands,
) {
    let player_chunk = get_chunk_index(player_transform.single().unwrap().translation);
    let lower_x = player_chunk.x  - COLLIDER_PRUNE_DISTANCE as i32;
    let upper_x = player_chunk.x  + COLLIDER_PRUNE_DISTANCE as i32;
    let lower_z = player_chunk.z  - COLLIDER_PRUNE_DISTANCE as i32;
    let upper_z = player_chunk.z  + COLLIDER_PRUNE_DISTANCE as i32;

    for (entity, transform) in colliders_transform {
        let curr_chunk = get_chunk_index(transform.translation);
        if !(lower_x <= curr_chunk.x
            && curr_chunk.x <= upper_x
            && lower_z <= curr_chunk.z
            && curr_chunk.z <= upper_z)
        {
            commands.entity(entity).remove::<Collider>();
        }
    }
}
