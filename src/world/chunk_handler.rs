use crate::plugins::{movement::Movement, player::Player};
use crate::world::generate_chunk;
use crate::world::{
    CHUNK_RENDER_DISTANCE, CHUNK_WORLD_SIZE, CHUNK_Y_COUNT, ChunkChannel, ChunkEntities,
    ChunkLoadInfo, ChunkScheduler, ChunkVoxels, LastChunk, MAX_CONCURRENT_CHUNK_JOBS,
    RunningChunkJob, TerrainMaterial, ToBeInvalidatedChunks, VoxelData, chunk_view_generator,
    chunk_world_origin, generate_mesh, get_lod, xz_chunk_manhattan_distance,
};
use bevy::ecs::relationship::RelationshipSourceCollection;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::tasks::AsyncComputeTaskPool;

use std::sync::Mutex;
use std::sync::mpsc::channel;

const MAX_ASYNC_RESULTS_PER_FRAME: usize = 7; // with no throttles, usually ~150 chunks per frame, only ever want a total of ~300 being processed total
const SYNC_CHUNK_DISTANCE: u32 = 1;

#[derive(Resource, Default)]
struct SynchronousChunkLoads {
    queue: Vec<(ChunkLoadInfo, Option<Mesh>, VoxelData)>,
}

#[derive(SystemParam)]
struct ProcessChunkMeshesParams<'w, 's> {
    scheduler: ResMut<'w, ChunkScheduler>,
    chunk_channel: Res<'w, ChunkChannel>,
    meshes: ResMut<'w, Assets<Mesh>>,
    terrain_material: Res<'w, TerrainMaterial>,
    chunk_entities: ResMut<'w, ChunkEntities>,
    chunk_voxels: ResMut<'w, ChunkVoxels>,
    to_be_invalidated: ResMut<'w, ToBeInvalidatedChunks>,
    synchronous_loads: ResMut<'w, SynchronousChunkLoads>,
    commands: Commands<'w, 's>,
}

pub struct ChunkHandlerPlugin;
impl Plugin for ChunkHandlerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, set_up_chunk_async);
        app.init_resource::<ChunkScheduler>();
        app.init_resource::<SynchronousChunkLoads>();
        app.add_systems(
            Update,
            (
                (prune_chunks, update_chunk_lods, request_desired_chunks)
                    .chain()
                    .run_if(chunk_changed),
                load_synchronous_chunks.run_if(chunk_changed),
                dispatch_chunk_jobs,
                process_chunk_meshes.after(load_synchronous_chunks),
            ),
        );
        app.add_systems(PostUpdate, update_last_chunk);
    }
}

fn set_up_chunk_async(mut command: Commands) {
    let (tx, rx) = channel::<(ChunkLoadInfo, Option<Mesh>, VoxelData)>();
    command.insert_resource(ChunkChannel {
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

fn prune_chunks(
    single: Single<Movement, With<Player>>,
    mut chunk_entities: ResMut<ChunkEntities>,
    mut chunk_voxels: ResMut<ChunkVoxels>,
    mut to_be_invalidated: ResMut<ToBeInvalidatedChunks>,
    mut scheduler: ResMut<ChunkScheduler>,
    mut commands: Commands,
) {
    let (transform, _, _) = single.into_inner();
    let curr_chunk = get_chunk_index(transform.translation);
    let mut to_despawn: Vec<Entity> = Vec::new();
    let min_x = curr_chunk.x - CHUNK_RENDER_DISTANCE;
    let max_x = curr_chunk.x + CHUNK_RENDER_DISTANCE;
    let min_z = curr_chunk.z - CHUNK_RENDER_DISTANCE;
    let max_z = curr_chunk.z + CHUNK_RENDER_DISTANCE;

    let in_bounds =
        |key: &IVec3| key.x >= min_x && key.x <= max_x && key.z >= min_z && key.z <= max_z;

    chunk_entities.chunks.retain(|key, entities| {
        let keep = in_bounds(key);
        if !keep {
            to_despawn.extend(entities.iter());
            chunk_voxels.chunks.remove(key);
        }
        keep
    });

    to_be_invalidated.chunks.retain(|key, entities| {
        let keep = in_bounds(key);
        if !keep {
            to_despawn.extend(entities.iter());
            chunk_voxels.chunks.remove(key);
        }
        keep
    });

    scheduler.latest.retain(|key, _| in_bounds(key));
    scheduler.in_flight.retain(|key, _| in_bounds(key));

    for entity in to_despawn {
        if let Ok(mut e) = commands.get_entity(entity) {
            e.despawn();
        }
    }
}

fn update_chunk_lods(
    single: Single<Movement, With<Player>>,
    mut scheduler: ResMut<ChunkScheduler>,
    mut chunk_entities: ResMut<ChunkEntities>,
    chunk_voxels: Res<ChunkVoxels>,
    mut to_be_invalidated: ResMut<ToBeInvalidatedChunks>,
) {
    let (transform, _, _) = single.into_inner();
    let curr_chunk = get_chunk_index(transform.translation);
    let to_remesh: Vec<(IVec3, u8, bool)> = chunk_voxels
        .chunks
        .iter()
        .filter_map(|(&pos, voxel_data)| {
            let desired_lod = get_lod(curr_chunk, pos);
            if desired_lod == voxel_data.lod {
                None
            } else {
                Some((pos, desired_lod, desired_lod < voxel_data.lod))
            }
        })
        .collect();

    for (pos, lod, is_down_sample) in to_remesh {
        if scheduler
            .latest
            .get(&pos)
            .is_some_and(|job| job.lod == lod && job.is_replacing)
        {
            continue;
        }
        if let Some(entities) = chunk_entities.chunks.remove(&pos) {
            to_be_invalidated.chunks.insert(pos, entities);
        }
        scheduler.request(pos, lod, true, is_down_sample, curr_chunk);
    }
}

fn request_desired_chunks(
    single: Single<Movement, With<Player>>,
    chunk_entities: Res<ChunkEntities>,
    to_be_invalidated: Res<ToBeInvalidatedChunks>,
    mut scheduler: ResMut<ChunkScheduler>,
) {
    let (transform, _, _) = single.into_inner();
    let curr_chunk = get_chunk_index(transform.translation);

    for x in -CHUNK_RENDER_DISTANCE..=CHUNK_RENDER_DISTANCE {
        for z in -CHUNK_RENDER_DISTANCE..=CHUNK_RENDER_DISTANCE {
            for y in 0..CHUNK_Y_COUNT {
                let chunk_pos = IVec3::new(curr_chunk.x + x, y, curr_chunk.z + z);
                if chunk_entities.chunks.contains_key(&chunk_pos)
                    || to_be_invalidated.chunks.contains_key(&chunk_pos)
                    || scheduler.in_flight.contains_key(&chunk_pos)
                {
                    continue;
                }
                let lod = get_lod(curr_chunk, chunk_pos);
                scheduler.request(chunk_pos, lod, false, false, curr_chunk);
            }
        }
    }
}

fn dispatch_chunk_jobs(
    single: Single<Movement, With<Player>>,
    mut scheduler: ResMut<ChunkScheduler>,
    chunk_channel: Res<ChunkChannel>,
) {
    let (transform, _, _) = single.into_inner();
    let curr_chunk = get_chunk_index(transform.translation);

    while scheduler.in_flight.len() < MAX_CONCURRENT_CHUNK_JOBS {
        let Some(request) = scheduler.pop_next_valid(curr_chunk) else {
            break;
        };

        scheduler.in_flight.insert(
            request.pos,
            RunningChunkJob {
                lod: request.lod,
                job_id: request.job_id,
                is_replacing: request.is_replacing,
            },
        );

        let tx = chunk_channel.sender.clone();
        let load_info = ChunkLoadInfo {
            pos: request.pos,
            lod: request.lod,
            is_replacing: request.is_replacing,
            job_id: request.job_id,
        };

        AsyncComputeTaskPool::get()
            .spawn(async move {
                let interior_chunk = generate_chunk(load_info.pos, load_info.lod);
                let mut chunk_views = chunk_view_generator(&interior_chunk);
                let mesh = generate_mesh(&mut chunk_views, &interior_chunk);
                let _ = tx.send((load_info, mesh, interior_chunk));
            })
            .detach();
    }
}

fn load_synchronous_chunks(
    single: Single<Movement, With<Player>>,
    mut scheduler: ResMut<ChunkScheduler>,
    mut synchronous_loads: ResMut<SynchronousChunkLoads>,
    chunk_entities: Res<ChunkEntities>,
    to_be_invalidated: Res<ToBeInvalidatedChunks>,
) {
    let (transform, _, _) = single.into_inner();
    let curr_chunk = get_chunk_index(transform.translation);

    for chunk_pos in synchronous_chunk_positions(curr_chunk) {
        if chunk_entities.chunks.contains_key(&chunk_pos)
            && !to_be_invalidated.chunks.contains_key(&chunk_pos)
        {
            continue;
        }

        let is_replacing = to_be_invalidated.chunks.contains_key(&chunk_pos);
        let request = scheduler.request(chunk_pos, 0, is_replacing, false, curr_chunk);
        scheduler.in_flight.insert(
            chunk_pos,
            RunningChunkJob {
                lod: request.lod,
                job_id: request.job_id,
                is_replacing: request.is_replacing,
            },
        );

        let load_info = ChunkLoadInfo {
            pos: chunk_pos,
            lod: 0,
            is_replacing,
            job_id: request.job_id,
        };

        let interior_chunk = generate_chunk(chunk_pos, load_info.lod);
        let mut chunk_views = chunk_view_generator(&interior_chunk);
        let mesh = generate_mesh(&mut chunk_views, &interior_chunk);
        synchronous_loads
            .queue
            .push((load_info, mesh, interior_chunk));
    }
}

fn synchronous_chunk_positions(curr_chunk: IVec3) -> impl Iterator<Item = IVec3> {
    let distance = SYNC_CHUNK_DISTANCE as i32;
    (-distance..=distance).flat_map(move |x| {
        (-distance..=distance).flat_map(move |y| {
            (-distance..=distance).filter_map(move |z| {
                let chunk_pos = IVec3::new(curr_chunk.x + x, curr_chunk.y + y, curr_chunk.z + z);
                (xz_chunk_manhattan_distance(curr_chunk, chunk_pos) <= SYNC_CHUNK_DISTANCE)
                    .then_some(chunk_pos)
            })
        })
    })
}

fn process_chunk_meshes(single: Single<Movement, With<Player>>, params: ProcessChunkMeshesParams) {
    let ProcessChunkMeshesParams {
        mut scheduler,
        chunk_channel,
        mut meshes,
        terrain_material,
        mut chunk_entities,
        mut chunk_voxels,
        mut to_be_invalidated,
        mut synchronous_loads,
        mut commands,
    } = params;

    let (transform, _, _) = single.into_inner();
    let curr_chunk = get_chunk_index(transform.translation);

    let mut received: Vec<_> = synchronous_loads.queue.drain(..).collect();
    let async_received: Vec<_> = {
        let rx = chunk_channel.receiver.lock().unwrap();
        (0..MAX_ASYNC_RESULTS_PER_FRAME)
            .map_while(|_| rx.try_recv().ok())
            .collect()
    };
    received.extend(async_received);

    for (load_info, mesh, voxel_data) in received {
        let chunk_pos = load_info.pos;

        if !is_current_chunk_job(&scheduler, load_info) {
            scheduler.in_flight.remove(&chunk_pos);
            continue;
        }

        let in_bounds = (chunk_pos.x - curr_chunk.x).abs() <= CHUNK_RENDER_DISTANCE
            && (chunk_pos.z - curr_chunk.z).abs() <= CHUNK_RENDER_DISTANCE;

        if !in_bounds {
            if load_info.is_replacing {
                restore_invalidated_chunk(&mut chunk_entities, &mut to_be_invalidated, chunk_pos);
            }
            finish_chunk_job(&mut scheduler, load_info);
            continue;
        }

        if chunk_entities.chunks.contains_key(&chunk_pos) && !load_info.is_replacing {
            finish_chunk_job(&mut scheduler, load_info);
            continue;
        }

        let expected_lod = get_lod(curr_chunk, chunk_pos);
        if voxel_data.lod != expected_lod {
            if load_info.is_replacing {
                restore_invalidated_chunk(&mut chunk_entities, &mut to_be_invalidated, chunk_pos);
            }
            finish_chunk_job(&mut scheduler, load_info);
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
        finish_chunk_job(&mut scheduler, load_info);
    }
}

fn is_current_chunk_job(scheduler: &ChunkScheduler, load_info: ChunkLoadInfo) -> bool {
    scheduler
        .in_flight
        .get(&load_info.pos)
        .is_some_and(|job| job.lod == load_info.lod && job.job_id == load_info.job_id)
}

fn finish_chunk_job(scheduler: &mut ChunkScheduler, load_info: ChunkLoadInfo) {
    if is_current_chunk_job(scheduler, load_info) {
        scheduler.in_flight.remove(&load_info.pos);
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
