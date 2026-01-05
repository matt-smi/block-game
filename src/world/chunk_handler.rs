use bevy::prelude::*;
use crate::plugins::{movement::Movement, player::Player};
use crate::common::GameState;
use crate::world::{ChunkEntities, LastChunk, VoxelMapping, chunk_view_generator, generate_mesh, generate_no_padding_dumby_chunk}; 

const CHUNK_LOAD_DISTANCE: i32 = 50;

pub struct ChunkHandlerPlugin; 
impl Plugin for ChunkHandlerPlugin { 
    fn build(&self, app: &mut App) {
        app.add_systems(
                Update,
                (
                    prune_chunks.run_if(in_state(GameState::Playing).and(chunk_changed)),
                    load_chunks.run_if(chunk_changed),            //.after(prune_chunks),
                ),
            );
        app.add_systems(
            PostUpdate, 
        update_last_chunk);
        
    }
}

fn chunk_changed(player: Single<&Transform, With<Player>>, last_chunk: Option<Res<LastChunk>>) -> bool { 
    let player_position = player.translation;
    let curr_chunk = get_chunk_index(player_position);

     if let Some(last_chunk) = last_chunk {
        if curr_chunk == last_chunk.chunkPos { 
            return false;
        }
    }
    return true 
}

//curr_chunk: IVec3, chunk_entities: &mut HashMap<IVec3, Vec<Entity>>, 
fn prune_chunks(single: Single<Movement, With<Player>>, mut chunk_entities: ResMut<ChunkEntities>, last_chunk: Option<ResMut<LastChunk>>, mut commands: Commands){ 
    
    let (transform, _, _) = single.into_inner(); 
    let curr_chunk = get_chunk_index(transform.translation);
    
    println!("Chunks in view {:?}", chunk_entities.chunks.len());
    chunk_entities.chunks.retain(|key, entities| { 
        let in_bounds = (curr_chunk.x - key.x).pow(2) + (curr_chunk.z - key.z).pow(2) <= CHUNK_LOAD_DISTANCE.pow(2); 
        if !in_bounds { 
            for entity in entities { 
                commands.entity(*entity).despawn();
            }
        }
        in_bounds
    });
}


fn load_chunks(single: Single<Movement, With<Player>>, mapping: Res<VoxelMapping>, mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>, mut chunk_entities: ResMut<ChunkEntities>, mut commands: Commands){ 

    let (transform, _, _) = single.into_inner(); 
    let curr_chunk = get_chunk_index(transform.translation);
   
    for x in -CHUNK_LOAD_DISTANCE..(CHUNK_LOAD_DISTANCE + 1) {for z in -CHUNK_LOAD_DISTANCE..(CHUNK_LOAD_DISTANCE + 1) {
        let new_chunk_pos = IVec3::new(curr_chunk.x + x, 0, curr_chunk.z + z); 
        if chunk_entities.chunks.contains_key(&new_chunk_pos) { 
            println!("continuing");
            continue; 
        }
        println!("generating at {:?}", Vec3::new(16.0 * new_chunk_pos.x as f32, 0.0, 16.0 * new_chunk_pos.z as f32));
        
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

            chunk_entities.chunks.insert(new_chunk_pos, vec![entity]);
        }

    }}

    
}

fn update_last_chunk(player: Single<&Transform, With<Player>>, mut commands: Commands) { 
    let curr_chunk = get_chunk_index(player.translation);
    commands.insert_resource(LastChunk { 
        chunkPos: curr_chunk,
    });
}



fn get_chunk_index(world_position: Vec3) -> IVec3 { 
    IVec3::new((world_position.x / 16.0).floor() as i32, 0, (world_position.z / 16.0).floor() as i32)
}

