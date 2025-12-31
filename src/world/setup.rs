use bevy::prelude::*;

pub struct WorldPlugin;
impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (init_resources, setup.after(init_resources)));
    }
}

#[derive(Resource)]
pub struct VoxelResource {
    pub materials: Vec<Handle<StandardMaterial>>,
    pub mesh: Handle<Mesh>,
}

pub fn init_resources(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    commands.insert_resource(VoxelResource {
        materials: (0..10)
            .map(|i| {
                let hue = (i as f32 / 10.0) * 360.0;
                let color = Color::hsl(hue, 0.8, 0.5);
                materials.add(StandardMaterial {
                    base_color: color,
                    ..default()
                })
            })
            .collect(),
        mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
    });
}

fn setup(mut commands: Commands, voxel: Res<VoxelResource>) {
    let mut count = 0;
    let width = 100;
    let length = 100;

    for x in 0..=width {
        for z in 0..=length {
            commands.spawn((
                Mesh3d(voxel.mesh.clone()),
                MeshMaterial3d(voxel.materials[count % voxel.materials.len()].clone()),
                Transform::from_xyz(x as f32, 0.0, z as f32),
            ));
            count += 1;
        }
    }
}
