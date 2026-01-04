use bevy::{platform::collections::HashMap, prelude::*};
use leafwing_input_manager::prelude::ActionState;

use crate::common::GameAction;
use crate::plugins::{movement::Movement, player::Player};
use crate::common::GameState;
use crate::world::{ChunkEntities, LastChunk, VoxelMapping, chunk_view_generator, generate_mesh, generate_no_padding_dumby_chunk}; 

const CHUNK_LOAD_DISTANCE: i32 = 16;

pub struct ChunkHandlerPlugin; 
//after(init_resources)) and player spawn.
impl Plugin for ChunkHandlerPlugin { 
    fn build(&self, app: &mut App) {
        app.add_systems(
                Update,
                (update_chunk).run_if(in_state(GameState::Playing)), //add systems separately with guard in condition
            );
    }
}

fn update_chunk(single: Single<Movement, With<Player>>, mut chunk_entities: ResMut<ChunkEntities>, last_chunk: Option<ResMut<LastChunk>>, mut commands: Commands, mapping: Res<VoxelMapping>,  mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>){
    let (transform, _, _) = single.into_inner(); 
    let player_position = transform.translation; 
    let curr_chunk = IVec3::new((player_position.x / 16.0).floor() as i32, 0, (player_position.z / 16.0).floor() as i32);
    if let Some(last_chunk) = last_chunk {
        println!("{:?}", last_chunk.chunkPos);
        if curr_chunk == last_chunk.chunkPos { 
            return;
        }
    }

    prune_chunks(curr_chunk, &mut chunk_entities.chunks);
    load_chunks(curr_chunk, &mut chunk_entities.chunks, commands, mapping, materials, meshes);    
}



fn prune_chunks(curr_chunk: IVec3, chunk_entities: &mut HashMap<IVec3, Vec<Entity>>){ 
    //check entites that arent in use
    chunk_entities.retain(|key, value| { 
        (curr_chunk.x - key.x).pow(2) + (curr_chunk.z - key.z).pow(2) <= CHUNK_LOAD_DISTANCE.pow(2)
    });
}


//pass in cache as second argument.
// replace set-up.
fn load_chunks(curr_chunk: IVec3, mut chunk_entities: &mut HashMap<IVec3, Vec<Entity>>, mut commands: Commands, mapping: Res<VoxelMapping>, mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>){ 
   
    for x in -CHUNK_LOAD_DISTANCE..(CHUNK_LOAD_DISTANCE + 1) {for z in -CHUNK_LOAD_DISTANCE..(CHUNK_LOAD_DISTANCE + 1) {
        let new_chunk_pos = IVec3::new(curr_chunk.x + x, 0, curr_chunk.z + curr_chunk.z); 
        if chunk_entities.contains_key(&new_chunk_pos) { 
            continue; 
        }

        let interior_chunk = generate_no_padding_dumby_chunk();
   
        let mut chunk_views = chunk_view_generator(&interior_chunk);

        if let Some(mesh) = generate_mesh(&mut chunk_views, &mapping, &interior_chunk) {
            let entity = commands.spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(1.0, 1.0, 1.0),
                    ..default()
                })),
                Transform::from_xyz(16.0 * new_chunk_pos.x as f32, 0.0, 16.0 * new_chunk_pos.z as f32)
                    .with_scale(Vec3::splat(0.5)),
            )).id();

            chunk_entities.insert(IVec3::new(x, 0, z), vec![entity]);
        }

    }
     commands.insert_resource(LastChunk { 
        chunkPos: curr_chunk,
    });
    }
}


