use bevy::{
    asset::RenderAssetUsages,
    image::{ImageSampler},
    prelude::*,
    render::render_resource::{
        Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension,
    },
};

/// 0 = midnight, 0.25 = sunrise, 0.5 = noon, 0.75 = sunset.
#[derive(Resource)]
pub struct TimeOfDay {
    pub fraction: f32,
    pub seconds_per_day: f32,
}

impl Default for TimeOfDay {
    fn default() -> Self {
        Self {
            fraction: 0.75,
            seconds_per_day: 300.0,
        }
    }
}

const FACE_SIZE: u32 = 512;

/// Directional light transform for a time-of-day fraction.
pub fn sun_rotation(fraction: f32) -> Quat {
    let angle = fraction * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
    Quat::from_euler(EulerRot::XYZ, -0.35 + angle.sin() * 1.15, 0.4, 0.0)
}

pub fn moon_rotation(fraction: f32) -> Quat {
    sun_rotation(fraction) * Quat::from_rotation_y(std::f32::consts::PI)
}

/// Unit vector pointing toward the sun in world space (for lighting alignment).
pub fn sun_direction(fraction: f32) -> Vec3 {
    sun_rotation(fraction) * Vec3::Z
}

pub fn sun_height(fraction: f32) -> f32 {
    let angle = fraction * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
    angle.sin()
}

pub fn night_strength(fraction: f32) -> f32 {
    smoothstep(-0.22, -0.04, -sun_height(fraction))
}

/// Procedural cubemap: sunset gradient and soft horizon.
pub fn generate_sunset_cubemap(sun_dir: Vec3) -> Image {
    let sun_dir = sun_dir.normalize_or_zero();
    let mut data = Vec::with_capacity((FACE_SIZE * FACE_SIZE * 6 * 4) as usize);

    for face in 0..6 {
        for y in 0..FACE_SIZE {
            for x in 0..FACE_SIZE {
                let u = (x as f32 + 0.5) / FACE_SIZE as f32;
                let v = (y as f32 + 0.5) / FACE_SIZE as f32;
                let dir = cubemap_direction(face, u, v).normalize();
                let rgb = sky_rgb(dir, sun_dir);
                data.extend_from_slice(&[
                    (rgb.x * 255.0) as u8,
                    (rgb.y * 255.0) as u8,
                    (rgb.z * 255.0) as u8,
                    255,
                ]);
            }
        }
    }

    let mut image = Image::new(
        Extent3d {
            width: FACE_SIZE,
            height: FACE_SIZE * 6,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.reinterpret_stacked_2d_as_array(6);
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    image.sampler = ImageSampler::linear();
    image
}

fn sky_rgb(dir: Vec3, sun_dir: Vec3) -> Vec3 {
    let elevation = dir.y;
    let horizon = (1.0 - elevation.max(0.0)).powf(1.8);

    let zenith = Vec3::new(0.12, 0.10, 0.32);
    let horizon_color = Vec3::new(0.92, 0.38, 0.22);

    let toward_sun = dir.dot(sun_dir).max(0.0);
    let warm_bias = toward_sun.powf(6.0);
    let horizon_tint = horizon_color.lerp(Vec3::new(1.0, 0.52, 0.18), warm_bias);

    let soft_glow = toward_sun.powf(40.0) * 0.12;

    let mut rgb = zenith.lerp(horizon_tint, horizon) + Vec3::splat(soft_glow);

    if elevation <= 0.02 {
        // Below horizon: dark haze, not void black (matches fog at the rim).
        let under = (-elevation).clamp(0.0, 1.0);
        let ground_haze = Vec3::new(0.18, 0.14, 0.22).lerp(horizon_tint * 0.35, 1.0 - under);
        rgb = rgb.lerp(ground_haze, under.powf(0.5));
    }

    rgb.clamp(Vec3::ZERO, Vec3::ONE)
}

fn cubemap_direction(face: u32, u: f32, v: f32) -> Vec3 {
    let s = 2.0 * u - 1.0;
    let t = 2.0 * v - 1.0;
    match face {
        0 => Vec3::new(1.0, -t, -s),
        1 => Vec3::new(-1.0, -t, s),
        2 => Vec3::new(s, 1.0, t),
        3 => Vec3::new(s, -1.0, -t),
        4 => Vec3::new(s, -t, 1.0),
        5 => Vec3::new(-s, -t, -1.0),
        _ => Vec3::Y,
    }
}

pub struct DayNightPalette {
    pub ambient_color: Color,
    pub ambient_brightness: f32,
    pub sun_illuminance: f32,
    pub sun_color: Color,
    pub moon_illuminance: f32,
    pub moon_color: Color,
    pub fog_color: Color,
    pub fog_light_color: Color,
}

pub fn day_night_palette(fraction: f32) -> DayNightPalette {
    let height = sun_height(fraction);
    let day = smoothstep(0.12, 0.42, height);
    let night = night_strength(fraction);
    let twilight = (1.0 - smoothstep(0.0, 0.38, height.abs())).max(0.0) * (1.0 - night * 0.7);

    let ambient_color = Color::srgb(0.10, 0.12, 0.28)
        .mix(&Color::srgb(0.82, 0.86, 0.94), day)
        .mix(&Color::srgb(0.95, 0.62, 0.42), twilight * 0.75);

    let ambient_brightness = lerp(50.0, 130.0, day) + twilight * 45.0;

    let sun_illuminance = lerp(1_200.0, 24_000.0, day) + twilight * 12_000.0;
    let sun_color = Color::srgb(0.55, 0.62, 0.85)
        .mix(&Color::srgb(1.0, 0.96, 0.88), day)
        .mix(&Color::srgb(1.0, 0.55, 0.28), twilight * 0.9);

    let moon_illuminance = lerp(0.0, 2_500.0, night) * (1.0 - day * 0.6);
    let moon_color = Color::srgb(0.55, 0.65, 0.95);

    let fog_color = Color::srgb(0.08, 0.09, 0.18)
        .mix(&Color::srgb(0.58, 0.68, 0.82), day)
        .mix(&Color::srgb(0.72, 0.48, 0.38), twilight * 0.8);

    let fog_light_color = Color::srgb(0.35, 0.40, 0.62)
        .mix(&Color::srgb(1.0, 0.92, 0.72), day)
        .mix(&Color::srgb(1.0, 0.65, 0.35), twilight)
        .mix(&Color::srgb(0.25, 0.30, 0.55), night * 0.5);

    DayNightPalette {
        ambient_color,
        ambient_brightness,
        sun_illuminance,
        sun_color,
        moon_illuminance,
        moon_color,
        fog_color,
        fog_light_color,
    }
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if edge0 >= edge1 {
        return (x >= edge1) as u8 as f32;
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}
