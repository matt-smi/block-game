use crate::plugins::{movement::Movement, player::Player};
use crate::world::{
    ChunkChannel, ChunkEntities, LastChunk, chunk_view_generator, generate_mesh,
    generate_no_padding_dumby_chunk,
};
use bevy::prelude::*;
use bevy::tasks::AsyncComputeTaskPool;
use std::sync::Mutex;
use std::sync::mpsc::channel;

const CHUNK_LOAD_DISTANCE: i32 = 64;

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
    let (tx, rx) = channel::<(IVec3, Mesh)>();
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
    println!("TRUEE");
    true
}

//curr_chunk: IVec3, chunk_entities: &mut HashMap<IVec3, Vec<Entity>>, maybe make this async too profile it.
fn prune_chunks(
    single: Single<Movement, With<Player>>,
    mut chunk_entities: ResMut<ChunkEntities>,
    mut commands: Commands,
) {
    let (transform, _, _) = single.into_inner();
    let curr_chunk = get_chunk_index(transform.translation);
    println!("curr_chunk {:?}", curr_chunk);
    println!("Chunks in view {:?}", chunk_entities.chunks.len());
    chunk_entities.chunks.retain(|key, entities| {
        let dx = (curr_chunk.x - key.x).abs();
        let dz = (curr_chunk.z - key.z).abs();

        // Keep if within square bounds
        let in_bounds = dx <= CHUNK_LOAD_DISTANCE && dz <= CHUNK_LOAD_DISTANCE;
        println!("in_bounds {:?}", in_bounds);
        if !in_bounds {
            for entity in entities {
                commands.entity(*entity).despawn();
            }
        }
        in_bounds
    });
}

// Maybe in the future make it so nearby chunks like 3x3 are blocking/synchronous to ensure the player is standing on something
fn load_chunks(
    single: Single<Movement, With<Player>>,
    _materials: ResMut<Assets<StandardMaterial>>,
    _meshes: ResMut<Assets<Mesh>>,
    chunk_channel: Res<ChunkChannel>,
    chunk_entities: Res<ChunkEntities>,
    _commands: Commands,
) {
    let (transform, _, _) = single.into_inner();
    let curr_chunk = get_chunk_index(transform.translation);

    for x in -CHUNK_LOAD_DISTANCE..=CHUNK_LOAD_DISTANCE {
        for z in -CHUNK_LOAD_DISTANCE..=CHUNK_LOAD_DISTANCE {
            let tx = chunk_channel.sender.clone();
            let new_chunk_pos = IVec3::new(curr_chunk.x + x, 0, curr_chunk.z + z);
            if chunk_entities.chunks.contains_key(&new_chunk_pos) {
                continue;
            }

            AsyncComputeTaskPool::get()
                .spawn(async move {
                    let interior_chunk = generate_no_padding_dumby_chunk();

                    let mut chunk_views = chunk_view_generator(&interior_chunk);
                    if let Some(mesh) = generate_mesh(&mut chunk_views, &interior_chunk) {
                        let _ = tx.send((new_chunk_pos, mesh));
                    }
                })
                .detach();
        }
    }
}

fn process_chunk_meshes(
    chunk_channel: Res<ChunkChannel>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut chunk_entities: ResMut<ChunkEntities>,
    mut commands: Commands,
) {
    let rx = chunk_channel.receiver.lock().unwrap();

    // Process all completed chunks this frame
    while let Ok((chunk_pos, mesh)) = rx.try_recv() {
        let entity = commands
            .spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(1.0, 1.0, 1.0),
                    ..default()
                })),
                Transform::from_xyz(16.0 * chunk_pos.x as f32, 0.0, 16.0 * chunk_pos.z as f32)
                    .with_scale(Vec3::splat(0.5)),
            ))
            .id();

        chunk_entities.chunks.insert(chunk_pos, vec![entity]);
    }
}

fn update_last_chunk(player: Single<&Transform, With<Player>>, mut commands: Commands) {
    let curr_chunk = get_chunk_index(player.translation);
    commands.insert_resource(LastChunk {
        chunk_pos: curr_chunk,
    });
}

fn get_chunk_index(world_position: Vec3) -> IVec3 {
    IVec3::new(
        (world_position.x / 16.0).floor() as i32,
        0,
        (world_position.z / 16.0).floor() as i32,
    )
}
