use crate::plugins::{movement::Movement, player::Player};
use crate::world::{
    CHUNK_WORLD_SIZE, CHUNK_Y_COUNT, CHUNK_RENDER_DISTANCE, ChunkChannel, ChunkEntities, ChunkVoxels, LastChunk, VoxelData,
    chunk_view_generator, chunk_world_origin, generate_mesh, get_lod 
};
use crate::world::generate_chunk;
use bevy::ecs::relationship::RelationshipSourceCollection;
use bevy::prelude::*;
use bevy::tasks::AsyncComputeTaskPool;
use std::sync::Mutex;
use std::sync::mpsc::channel;
use std::collections::HashSet;
use tokio::time::Instant;

// TODO: Look into using commandQueue instead of mpsc:channel.
pub struct ChunkHandlerPlugin;
impl Plugin for ChunkHandlerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, set_up_chunk_async);
        app.add_systems(
            Update,
            (
                (load_chunks, prune_chunks).chain().run_if(chunk_changed),
                process_chunk_meshes,
            ),
        );
        app.add_systems(PostUpdate, update_last_chunk);
    }
}

fn set_up_chunk_async(mut command: Commands) {
    let (tx, rx) = channel::<(IVec3, Mesh, VoxelData)>();
    command.insert_resource(ChunkChannel {
        processing_queue: HashSet::new(),
        sender: tx,
        receiver: Mutex::new(rx),
    });
}

fn chunk_changed(
    player: Single<&Transform, With<Player>>,
    last_chunk: Option<Res<LastChunk>>,
) -> bool {
    let player_position = player.translation;
    let curr_chunk = get_chunk_index(player_position);

    if let Some(last_chunk) = last_chunk
        && curr_chunk == last_chunk.chunk_pos
    {
        return false;
    }
    true
}

//curr_chunk: IVec3, chunk_entities: &mut HashMap<IVec3, Vec<Entity>>, maybe make this async too profile it.
// TODO: look into performance for this
fn prune_chunks(
    single: Single<Movement, With<Player>>,
    mut chunk_entities: ResMut<ChunkEntities>,
    mut chunk_voxels: ResMut<ChunkVoxels>,
    mut commands: Commands,
) {
    let (transform, _, _) = single.into_inner();
    let curr_chunk = get_chunk_index(transform.translation);
    let mut to_despawn: Vec<Entity> = Vec::new();
    let min_x = curr_chunk.x - CHUNK_RENDER_DISTANCE;
    let max_x = curr_chunk.x + CHUNK_RENDER_DISTANCE;
    let min_z = curr_chunk.z - CHUNK_RENDER_DISTANCE;
    let max_z = curr_chunk.z + CHUNK_RENDER_DISTANCE;

    chunk_entities.chunks.retain(|key, entities| {
        let in_bounds = key.x >= min_x && key.x <= max_x && key.z >= min_z && key.z <= max_z;

        if !in_bounds {
            to_despawn.extend(entities.iter());
            chunk_voxels.chunks.remove(key); // we have a case in which we remove the key but never delete??
        }
        in_bounds
    });
    
    for entity in to_despawn {
    if let Ok(mut e) = commands.get_entity(entity) { 
        e.despawn();
    } else {
        println!("we did not despawn but removed the key??");
    }
}
}

// Maybe in the future make it so nearby chunks like 3x3 are blocking/synchronous to ensure the player is standing on something
fn load_chunks(
    single: Single<Movement, With<Player>>,
    mut chunk_channel: ResMut<ChunkChannel>,
    chunk_entities: Res<ChunkEntities>,
    _commands: Commands,
) {
    let (transform, _, _) = single.into_inner();
    let curr_chunk = get_chunk_index(transform.translation);

    let mut positions: Vec<(IVec3, u8)> = (-CHUNK_RENDER_DISTANCE..=CHUNK_RENDER_DISTANCE)
    .flat_map(|x| {
        (-CHUNK_RENDER_DISTANCE..=CHUNK_RENDER_DISTANCE)
            .flat_map(move |z| (0..CHUNK_Y_COUNT).map(move |y| {
                let chunk_idx = IVec3::new(curr_chunk.x + x, y, curr_chunk.z + z);
                (chunk_idx, get_lod(curr_chunk, chunk_idx))}))
    })
    .filter(|(chunk_idx, _)| !chunk_entities.chunks.contains_key(chunk_idx))
    .collect();

    positions.sort_unstable_by_key(|(pos, _)| {
        let dx = pos.x - curr_chunk.x;
        let dz = pos.z - curr_chunk.z;
        dx * dx + dz * dz
    });

    for (new_chunk_pos, lod) in positions {
        let tx = chunk_channel.sender.clone();
        if chunk_channel.processing_queue.contains(&new_chunk_pos){ 
            continue
        }
        chunk_channel.processing_queue.insert(new_chunk_pos);
        AsyncComputeTaskPool::get()
            .spawn(async move {
                //let start = Instant::now();
                let interior_chunk = generate_chunk(new_chunk_pos, lod); 
                let mut chunk_views = chunk_view_generator(&interior_chunk); 
                if let Some(mesh) = generate_mesh(&mut chunk_views, &interior_chunk) { 
                    let _ = tx.send((new_chunk_pos, mesh, interior_chunk));
                }
                // let elapsed = start.elapsed();
                // debug!("Meshed chunk in {:.3?}", elapsed);
            })
            .detach();
    } 
}

// need to also make colliders async... after since current will always be synchronous, and async will be the close 6 neighbours...
fn process_chunk_meshes(
    single: Single<Movement, With<Player>>, 
    mut chunk_channel: ResMut<ChunkChannel>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut chunk_entities: ResMut<ChunkEntities>,
    mut chunk_voxels: ResMut<ChunkVoxels>,
    mut commands: Commands,
) {
    let (transform, _, _) = single.into_inner(); 
    let curr_chunk = get_chunk_index(transform.translation);

     let received: Vec<_> = {
        let rx = chunk_channel.receiver.lock().unwrap();
        (0..8)
            .map_while(|_| rx.try_recv().ok())
            .collect()
    };

    for (chunk_pos, mesh, voxel_data) in received {

        let in_bounds = (chunk_pos.x - curr_chunk.x).abs() <= CHUNK_RENDER_DISTANCE
            && (chunk_pos.z - curr_chunk.z).abs() <= CHUNK_RENDER_DISTANCE;

        if !in_bounds || chunk_entities.chunks.contains_key(&chunk_pos){ 
            continue;
        }

        let entity = commands
            .spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(1.0, 1.0, 1.0),
                    ..default()
                })),
                Transform::from_translation(chunk_world_origin(chunk_pos)),
            ))
            .id();

        chunk_entities.chunks.insert(chunk_pos, vec![entity]); 
        chunk_voxels.chunks.insert(chunk_pos, voxel_data);
        chunk_channel.processing_queue.remove(&chunk_pos);
    }
}
//mesh can be generated from points not even mesh needed.
fn update_last_chunk(player: Single<&Transform, With<Player>>, mut commands: Commands) {
    let curr_chunk = get_chunk_index(player.translation);
    commands.insert_resource(LastChunk {
        chunk_pos: curr_chunk,
    });
}

pub fn get_chunk_index(world_position: Vec3) -> IVec3 {
    IVec3::new(
        (world_position.x / CHUNK_WORLD_SIZE).floor() as i32,
        0,
        (world_position.z / CHUNK_WORLD_SIZE).floor() as i32,
    )
}
