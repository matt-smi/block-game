use crate::plugins::{movement::Movement, player::Player};
use crate::world::{
    CHUNK_WORLD_SIZE, ChunkChannel, ChunkEntities, ChunkVoxels, LastChunk, VoxelData,
    chunk_view_generator, chunk_world_origin, generate_mesh, generate_no_padding_dumby_chunk,
};
use bevy::ecs::relationship::RelationshipSourceCollection;
use bevy::prelude::*;
use bevy::tasks::AsyncComputeTaskPool;
use std::sync::Mutex;
use std::sync::mpsc::channel;

const CHUNK_LOAD_DISTANCE: i32 = 30;
const CHUNK_Y_COUNT: i32 = 10;

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
    let min_x = curr_chunk.x - CHUNK_LOAD_DISTANCE;
    let max_x = curr_chunk.x + CHUNK_LOAD_DISTANCE;
    let min_z = curr_chunk.z - CHUNK_LOAD_DISTANCE;
    let max_z = curr_chunk.z + CHUNK_LOAD_DISTANCE;

    chunk_entities.chunks.retain(|key, entities| {
        let in_bounds = key.x >= min_x && key.x <= max_x && key.z >= min_z && key.z <= max_z;

        if !in_bounds {
            to_despawn.extend(entities.iter());
            chunk_voxels.chunks.remove(key);
        }
        in_bounds
    });
    
    for entity in to_despawn {
    if let Ok(mut e) = commands.get_entity(entity) { 
        e.despawn();
    }
}
}

// Maybe in the future make it so nearby chunks like 3x3 are blocking/synchronous to ensure the player is standing on something
fn load_chunks(
    single: Single<Movement, With<Player>>,
    chunk_channel: Res<ChunkChannel>,
    chunk_entities: Res<ChunkEntities>,
    _commands: Commands,
) {
    let (transform, _, _) = single.into_inner();
    let curr_chunk = get_chunk_index(transform.translation);

    let mut positions: Vec<IVec3> = (-CHUNK_LOAD_DISTANCE..=CHUNK_LOAD_DISTANCE)
    .flat_map(|x| {
        (-CHUNK_LOAD_DISTANCE..=CHUNK_LOAD_DISTANCE)
            .flat_map(move |z| (0..CHUNK_Y_COUNT).map(move |y| IVec3::new(curr_chunk.x + x, y, curr_chunk.z + z)))
    })
    .filter(|pos| !chunk_entities.chunks.contains_key(pos))
    .collect();

    positions.sort_unstable_by_key(|pos| {
        let dx = pos.x - curr_chunk.x;
        let dz = pos.z - curr_chunk.z;
        dx * dx + dz * dz
    });

    for new_chunk_pos in positions {
        let tx = chunk_channel.sender.clone();
        AsyncComputeTaskPool::get()
            .spawn(async move {
                let interior_chunk = generate_no_padding_dumby_chunk();
                let mut chunk_views = chunk_view_generator(&interior_chunk);
                if let Some(mesh) = generate_mesh(&mut chunk_views, &interior_chunk) {
                    let _ = tx.send((new_chunk_pos, mesh, interior_chunk));
                }
            })
            .detach();
    } 
}

// need to also make colliders async... after since current will always be synchronous, and async will be the close 6 neighbours...
fn process_chunk_meshes(
    single: Single<Movement, With<Player>>, 
    chunk_channel: Res<ChunkChannel>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut chunk_entities: ResMut<ChunkEntities>,
    mut chunk_voxels: ResMut<ChunkVoxels>,
    mut commands: Commands,
) {
    let (transform, _, _) = single.into_inner(); 
    let rx = chunk_channel.receiver.lock().unwrap();
    let curr_chunk = get_chunk_index(transform.translation);
    let mut count: u8 = 0;
    while count < 4 && let Ok((chunk_pos, mesh, voxel_data)) = rx.try_recv() {

        let in_bounds = (chunk_pos.x - curr_chunk.x).abs() <= CHUNK_LOAD_DISTANCE
            && (chunk_pos.z - curr_chunk.z).abs() <= CHUNK_LOAD_DISTANCE;
        if !in_bounds {
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
        count += 1;
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
