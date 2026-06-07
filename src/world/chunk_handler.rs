use crate::plugins::{movement::Movement, player::Player};
use crate::world::generate_chunk;
use crate::world::{
    CHUNK_RENDER_DISTANCE, CHUNK_WORLD_SIZE, CHUNK_Y_COUNT, ChunkChannel, ChunkEntities,
    ChunkLoadInfo, ChunkVoxels, LastChunk, ProcessingChunk, TerrainMaterial, ToBeInvalidatedChunks,
    VoxelData, chunk_view_generator, chunk_world_origin, generate_mesh, get_lod,
};
use bevy::ecs::relationship::RelationshipSourceCollection;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::tasks::AsyncComputeTaskPool;
use std::cmp::Reverse;
use std::sync::Mutex;
use std::sync::mpsc::channel;

/*
    Good to keep this number pretty low, since if a player moves really fast we fill up the backlog
    with chunks that are invalid before they even mesh and increase the delay for chunk loading near the player
    (we could probably use a better system but I think this is probably good enough for now)
*/
const CHUNKS_TO_QUEUE_PER_FRAME: usize = 3;

#[derive(Resource, Default)]
struct PendingChunkLoads {
    queue: Vec<(IVec3, u8)>,
}

// TODO: Look into using commandQueue instead of mpsc:channel.
pub struct ChunkHandlerPlugin;
impl Plugin for ChunkHandlerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, set_up_chunk_async);
        app.init_resource::<PendingChunkLoads>();
        app.add_systems(
            Update,
            (
                (prune_chunks, update_chunk_lods)
                    .chain()
                    .run_if(chunk_changed),
            refill_pending_chunk_loads.run_if(chunk_changed),
                load_chunks,
                process_chunk_meshes,
            ),
        );
        app.add_systems(PostUpdate, update_last_chunk);
    }
}

fn set_up_chunk_async(mut command: Commands) {
    let (tx, rx) = channel::<(ChunkLoadInfo, Option<Mesh>, VoxelData)>();
    command.insert_resource(ChunkChannel {
        processing_queue: HashMap::new(),
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
    mut to_be_invalidated: ResMut<ToBeInvalidatedChunks>,
    mut chunk_channel: ResMut<ChunkChannel>,
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
            chunk_voxels.chunks.remove(key);
        }
        in_bounds
    });

    to_be_invalidated.chunks.retain(|key, entities| {
        let in_bounds = key.x >= min_x && key.x <= max_x && key.z >= min_z && key.z <= max_z;
        if !in_bounds {
            to_despawn.extend(entities.iter());
            chunk_voxels.chunks.remove(key);
        }
        in_bounds
    });

    chunk_channel
        .processing_queue
        .retain(|key, _| key.x >= min_x && key.x <= max_x && key.z >= min_z && key.z <= max_z);

    for entity in to_despawn {
        if let Ok(mut e) = commands.get_entity(entity) {
            e.despawn();
        } else {
            println!("we did not despawn but removed the key??");
        }
    }
}

fn update_chunk_lods(
    single: Single<Movement, With<Player>>,
    mut chunk_channel: ResMut<ChunkChannel>,
    mut chunk_entities: ResMut<ChunkEntities>,
    chunk_voxels: Res<ChunkVoxels>,
    mut to_be_invalidated: ResMut<ToBeInvalidatedChunks>,
) {
    let (transform, _, _) = single.into_inner();
    let curr_chunk = get_chunk_index(transform.translation);
    let to_remesh: Vec<(IVec3, u8)> = chunk_voxels
        .chunks
        .iter()
        .filter_map(|(&pos, voxel_data)| {
            let desired_lod = get_lod(curr_chunk, pos);
            if desired_lod == voxel_data.lod {
                None
            } else {
                Some((pos, desired_lod))
            }
        })
        .collect();

    for (pos, lod) in to_remesh {
        if chunk_channel
            .processing_queue
            .get(&pos)
            .is_some_and(|job| job.lod == lod)
        {
            continue;
        }
        if let Some(entities) = chunk_entities.chunks.remove(&pos) {
            to_be_invalidated.chunks.insert(pos, entities);
        }
        chunk_channel.processing_queue.remove(&pos);
        queue_chunk_mesh(&mut chunk_channel, pos, lod, true);
    }
}

fn build_pending_queue(
    curr_chunk: IVec3,
    chunk_entities: &ChunkEntities,
    to_be_invalidated: &ToBeInvalidatedChunks,
    processing_queue: &HashMap<IVec3, ProcessingChunk>,
) -> Vec<(IVec3, u8)> {
    let mut queue: Vec<(IVec3, u8)> = (-CHUNK_RENDER_DISTANCE..=CHUNK_RENDER_DISTANCE)
        .flat_map(|x| {
            (-CHUNK_RENDER_DISTANCE..=CHUNK_RENDER_DISTANCE).flat_map(move |z| {
                (0..CHUNK_Y_COUNT).map(move |y| {
                    let chunk_idx = IVec3::new(curr_chunk.x + x, y, curr_chunk.z + z);
                    (chunk_idx, get_lod(curr_chunk, chunk_idx))
                })
            })
        })
        .filter(|(chunk_idx, _)| {
            !chunk_entities.chunks.contains_key(chunk_idx)
                && !to_be_invalidated.chunks.contains_key(chunk_idx)
                && !processing_queue.contains_key(chunk_idx)
        })
        .collect();

    queue.sort_unstable_by_key(|(pos, _)| {
        let dx = pos.x - curr_chunk.x;
        let dz = pos.z - curr_chunk.z;
        Reverse(dx * dx + dz * dz)
    });
    queue
}

fn refill_pending_chunk_loads(
    single: Single<Movement, With<Player>>,
    chunk_entities: Res<ChunkEntities>,
    to_be_invalidated: Res<ToBeInvalidatedChunks>,
    chunk_channel: Res<ChunkChannel>,
    mut pending: ResMut<PendingChunkLoads>,
) {
    let (transform, _, _) = single.into_inner();
    let curr_chunk = get_chunk_index(transform.translation);
    pending.queue = build_pending_queue(
        curr_chunk,
        &chunk_entities,
        &to_be_invalidated,
        &chunk_channel.processing_queue,
    );
}

// Maybe in the future make it so nearby chunks like 3x3 are blocking/synchronous to ensure the player is standing on something
fn load_chunks(
    single: Single<Movement, With<Player>>,
    mut chunk_channel: ResMut<ChunkChannel>,
    chunk_entities: Res<ChunkEntities>,
    to_be_invalidated: Res<ToBeInvalidatedChunks>,
    mut pending: ResMut<PendingChunkLoads>,
) {
    if pending.queue.is_empty() {
        let (transform, _, _) = single.into_inner();
        let curr_chunk = get_chunk_index(transform.translation);
        pending.queue = build_pending_queue(
            curr_chunk,
            &chunk_entities,
            &to_be_invalidated,
            &chunk_channel.processing_queue,
        );
    }
    let mut queued = 0usize;
    while queued < CHUNKS_TO_QUEUE_PER_FRAME {
        let Some((new_chunk_pos, lod)) = pending.queue.pop() else {
            break;
        };
        if chunk_entities.chunks.contains_key(&new_chunk_pos)
            || to_be_invalidated.chunks.contains_key(&new_chunk_pos)
            || chunk_channel.processing_queue.contains_key(&new_chunk_pos)
        {
            continue;
        }
        queue_chunk_mesh(&mut chunk_channel, new_chunk_pos, lod, false);
        queued += 1;
    }
}

fn queue_chunk_mesh(
    chunk_channel: &mut ChunkChannel,
    chunk_pos: IVec3,
    lod: u8,
    is_replacing: bool,
) {
    if chunk_channel.processing_queue.contains_key(&chunk_pos) {
        return;
    }
    chunk_channel
        .processing_queue
        .insert(chunk_pos, ProcessingChunk { lod });
    let load_info = ChunkLoadInfo {
        pos: chunk_pos,
        lod,
        is_replacing,
    };
    let tx = chunk_channel.sender.clone();
    AsyncComputeTaskPool::get()
        .spawn(async move {
            let interior_chunk = generate_chunk(load_info.pos, lod);
            let mut chunk_views = chunk_view_generator(&interior_chunk);
            let mesh = generate_mesh(&mut chunk_views, &interior_chunk);
            let _ = tx.send((load_info, mesh, interior_chunk));
        })
        .detach();
}

fn process_chunk_meshes(
    single: Single<Movement, With<Player>>,
    mut chunk_channel: ResMut<ChunkChannel>,
    mut meshes: ResMut<Assets<Mesh>>,
    terrain_material: Res<TerrainMaterial>,
    mut chunk_entities: ResMut<ChunkEntities>,
    mut chunk_voxels: ResMut<ChunkVoxels>,
    mut to_be_invalidated: ResMut<ToBeInvalidatedChunks>,
    mut commands: Commands,
) {
    let (transform, _, _) = single.into_inner();
    let curr_chunk = get_chunk_index(transform.translation);

    let received: Vec<_> = {
        let rx = chunk_channel.receiver.lock().unwrap();
        (0..8).map_while(|_| rx.try_recv().ok()).collect()
    };

    for (load_info, mesh, voxel_data) in received {
        let chunk_pos = load_info.pos;

        if !is_current_chunk_job(&chunk_channel, load_info) {
            continue;
        }

        let in_bounds = (chunk_pos.x - curr_chunk.x).abs() <= CHUNK_RENDER_DISTANCE
            && (chunk_pos.z - curr_chunk.z).abs() <= CHUNK_RENDER_DISTANCE;

        if !in_bounds {
            if load_info.is_replacing {
                restore_invalidated_chunk(&mut chunk_entities, &mut to_be_invalidated, chunk_pos);
            }
            finish_chunk_job(&mut chunk_channel, load_info);
            continue;
        }

        if chunk_entities.chunks.contains_key(&chunk_pos) && !load_info.is_replacing {
            finish_chunk_job(&mut chunk_channel, load_info);
            continue;
        }

        let expected_lod = get_lod(curr_chunk, chunk_pos);
        if voxel_data.lod != expected_lod {
            if load_info.is_replacing {
                restore_invalidated_chunk(&mut chunk_entities, &mut to_be_invalidated, chunk_pos);
            }
            finish_chunk_job(&mut chunk_channel, load_info);
            continue;
        }

        if load_info.is_replacing {
            despawn_invalidated_chunk(&mut to_be_invalidated, &mut commands, chunk_pos);
        }

        let entities = if let Some(mesh) = mesh {
            let entity = commands
                .spawn((
                    Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(terrain_material.0.clone()),
                    Transform::from_translation(chunk_world_origin(chunk_pos)),
                ))
                .id();
            vec![entity]
        } else {
            Vec::new()
        };

        chunk_entities.chunks.insert(chunk_pos, entities);
        chunk_voxels.chunks.insert(chunk_pos, voxel_data);
        finish_chunk_job(&mut chunk_channel, load_info);
    }
}

fn is_current_chunk_job(chunk_channel: &ChunkChannel, load_info: ChunkLoadInfo) -> bool {
    chunk_channel
        .processing_queue
        .get(&load_info.pos)
        .is_some_and(|job| job.lod == load_info.lod)
}

fn finish_chunk_job(chunk_channel: &mut ChunkChannel, load_info: ChunkLoadInfo) {
    if is_current_chunk_job(chunk_channel, load_info) {
        chunk_channel.processing_queue.remove(&load_info.pos);
    }
}

fn despawn_invalidated_chunk(
    to_be_invalidated: &mut ToBeInvalidatedChunks,
    commands: &mut Commands,
    chunk_pos: IVec3,
) {
    if let Some(old_entities) = to_be_invalidated.chunks.remove(&chunk_pos) {
        for entity in old_entities {
            if let Ok(mut entity) = commands.get_entity(entity) {
                entity.despawn();
            }
        }
    }
}

fn restore_invalidated_chunk(
    chunk_entities: &mut ChunkEntities,
    to_be_invalidated: &mut ToBeInvalidatedChunks,
    chunk_pos: IVec3,
) {
    if let Some(entities) = to_be_invalidated.chunks.remove(&chunk_pos) {
        chunk_entities.chunks.insert(chunk_pos, entities);
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
