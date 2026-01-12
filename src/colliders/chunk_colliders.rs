use avian3d::prelude::{Collider, FillMode};
use bevy::prelude::*;

use std::collections::HashSet;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Mutex;

use crate::plugins::player::Player;
use crate::world::{ChunkEntities, get_chunk_index};

#[derive(Resource)]
pub struct ColliderChannel { 
    pub sender: Sender<Collider>, 
    pub reciever: Mutex<Receiver<(IVec3, Collider)>>

}

// TODO: Make nearby collider generation async. 

pub struct ChunkColliderPlugin; 

impl Plugin for ChunkColliderPlugin { 
    fn build(&self, app: &mut App ){ 
        app.add_systems(FixedUpdate, 
            (generate_colliders, prune_colliders)
        );
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
                if let Ok(mesh3d) = mesh_query.get(entity_id) {
                    if let Some(mesh) = meshes.get(&mesh3d.0) {
                        let collider = Collider::trimesh_from_mesh(mesh); 
                        if let Some(collider) = collider {
                            commands.entity(entity_id).insert(collider);
                        }
                    }
                }
            }
        }
    }
}

fn prune_colliders(){ 

}