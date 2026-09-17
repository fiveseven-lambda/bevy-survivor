use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use rand::RngExt;

const VIEWPORT_RATIO: UVec2 = uvec2(16, 9);
const VIEW_SIZE: Vec2 = vec2(64.0, 36.0);
const FIELD_SIZE: UVec2 = uvec2(1024, 576);
const FIELD_WIDTH: usize = FIELD_SIZE.x as usize;
const FIELD_HEIGHT: usize = FIELD_SIZE.y as usize;

const RESOURCE_COLORS: [Color; 3] = [
    Color::hsl(40.0, 0.8, 0.5),
    Color::hsl(160.0, 0.8, 0.5),
    Color::hsl(280.0, 0.8, 0.5),
];

#[derive(Resource)]
struct Field(Vec<Vec<u8>>);

#[derive(Resource)]
struct Player {
    position: Vec2,
    direction: Vec2,
    resources: [f32; 3],
}

#[derive(Component)]
struct PlayerStroke;

#[derive(Component)]
struct PlayerFill;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .insert_resource(Player {
            position: Vec2::ZERO,
            direction: Vec2::X,
            resources: [0.0; 3],
        })
        .add_systems(
            Update,
            update_viewport.run_if(on_message::<bevy::window::WindowResized>),
        )
        .add_systems(Update, update_direction.run_if(on_message::<CursorMoved>))
        .add_systems(Update, move_player)
        .run();
}

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    window: Single<&Window>,
) {
    commands.spawn((
        Camera2d,
        Camera {
            viewport: Some(calculate_viewport(&window)),
            ..default()
        },
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: bevy::camera::ScalingMode::Fixed {
                width: VIEW_SIZE.x,
                height: VIEW_SIZE.y,
            },
            ..OrthographicProjection::default_2d()
        }),
    ));

    let mut rng = rand::rng();
    let mut field_values: Vec<Vec<[f64; 3]>> = (0..FIELD_HEIGHT)
        .map(|_| (0..FIELD_WIDTH).map(|_| rng.random()).collect())
        .collect();
    for _ in 0..1000 {
        field_values = (0..FIELD_HEIGHT)
            .map(|i| {
                let up = if i == 0 { FIELD_HEIGHT - 1 } else { i - 1 };
                let down = if i == FIELD_HEIGHT - 1 { 0 } else { i + 1 };
                (0..FIELD_WIDTH)
                    .map(|j| {
                        let left = if j == 0 { FIELD_WIDTH - 1 } else { j - 1 };
                        let right = if j == FIELD_WIDTH - 1 { 0 } else { j + 1 };
                        let mut next_values = [0.0; 3];
                        for k in 0..3 {
                            let mut next_value = 10.0 * field_values[i][j][k];
                            next_value += 4.0 * field_values[i][left][k];
                            next_value += 4.0 * field_values[i][right][k];
                            next_value += 4.0 * field_values[up][j][k];
                            next_value += 4.0 * field_values[down][j][k];
                            next_value += field_values[up][left][k];
                            next_value += field_values[up][right][k];
                            next_value += field_values[down][left][k];
                            next_value += field_values[down][right][k];
                            next_values[k] = next_value / 30.0;
                        }
                        for (k1, k2) in [(0, 1), (1, 2), (2, 0)] {
                            let reaction = 0.01
                                * field_values[i][j][k1]
                                * field_values[i][j][k2]
                                * (field_values[i][j][k1] - field_values[i][j][k2]);
                            next_values[k1] += reaction;
                            next_values[k2] -= reaction;
                        }
                        next_values
                    })
                    .collect()
            })
            .collect();
    }

    let field: Vec<Vec<u8>> = field_values
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|values| {
                    let max_value = values.into_iter().reduce(f64::max).unwrap();
                    values
                        .iter()
                        .position(|&value| value == max_value)
                        .unwrap_or(0) as u8
                })
                .collect()
        })
        .collect();

    let mut field_image_data = Vec::new();
    for row in field.iter().rev() {
        for &cell in row {
            field_image_data.extend(RESOURCE_COLORS[cell as usize].to_linear().to_u8_array());
        }
    }

    let mut field_image = Image::new(
        Extent3d {
            height: FIELD_HEIGHT as u32,
            width: FIELD_WIDTH as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        field_image_data,
        TextureFormat::Rgba8Unorm,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    let mut field_image_sampler_descriptor = ImageSamplerDescriptor::nearest();
    field_image_sampler_descriptor.set_address_mode(ImageAddressMode::Repeat);
    field_image.sampler = ImageSampler::Descriptor(field_image_sampler_descriptor);

    let mut field_sprite = Sprite::from_image(images.add(field_image));
    field_sprite.rect = Some(Rect::from_center_size(Vec2::ZERO, VIEW_SIZE));
    commands.spawn((field_sprite, Transform::from_xyz(0.0, 0.0, -1.0)));

    let player_fill = Circle::new(1.0);
    let player_fill_color = RESOURCE_COLORS[field[0][0] as usize];
    commands.spawn((
        PlayerFill,
        Mesh2d(meshes.add(player_fill)),
        MeshMaterial2d(materials.add(player_fill_color)),
    ));

    let player_stroke = Circle::new(1.0).to_ring(0.1);
    let player_stroke_color = Color::WHITE;
    commands.spawn((
        PlayerStroke,
        Mesh2d(meshes.add(player_stroke)),
        MeshMaterial2d(materials.add(player_stroke_color)),
    ));

    commands.insert_resource(Field(field));

    commands.spawn((
        Text2d::new(""),
        bevy::sprite::Anchor::TOP_LEFT,
        Transform {
            translation: Vec3 {
                x: -VIEW_SIZE.x / 2.0,
                y: VIEW_SIZE.y / 2.0,
                z: 0.0,
            },
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(0.1),
        },
    ));
}

fn update_viewport(mut camera: Single<&mut Camera>, window: Single<&Window>) {
    camera.viewport = Some(calculate_viewport(&window));
}

fn calculate_viewport(window: &Window) -> bevy::camera::Viewport {
    let window_size = window.physical_size();
    let UVec2 { x, y } = window_size / VIEWPORT_RATIO;
    let size = VIEWPORT_RATIO * u32::min(x, y);
    let position = (window_size - size) / 2;
    bevy::camera::Viewport {
        physical_position: position,
        physical_size: size,
        ..default()
    }
}

fn update_direction(
    mut player: ResMut<Player>,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform)>,
) {
    let (camera, camera_transform) = camera.into_inner();
    if let Some(viewport_position) = window.cursor_position()
        && let Ok(world_position) = camera.viewport_to_world_2d(camera_transform, viewport_position)
    {
        player.direction = world_position;
    }
}

fn move_player(
    mut player: ResMut<Player>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut background: Single<&mut Sprite>,
    mut text: Single<&mut Text2d>,
    player_fill: Single<&MeshMaterial2d<ColorMaterial>, With<PlayerFill>>,
    field: Res<Field>,
    time: Res<Time>,
) {
    let pos_float = player.position + 10. * player.direction.normalize() * time.delta_secs();
    let pos_int = pos_float.as_ivec2();
    let field_size = FIELD_SIZE.as_ivec2();
    let q = pos_int.div_euclid(field_size) * field_size;
    player.position = pos_float - q.as_vec2();
    let IVec2 { x: column, y: row } = pos_int - q;
    let cell = field.0[row as usize][column as usize] as usize;
    player.resources[cell] += time.delta_secs();
    text.0 = format!(
        "{:.1}, {:.1}, {:.1}",
        player.resources[0], player.resources[1], player.resources[2],
    );
    let color = RESOURCE_COLORS[cell];
    materials.get_mut(player_fill.id()).unwrap().color = color;
    background.rect = Some(Rect::from_center_size(
        Vec2 {
            x: player.position.x,
            y: -player.position.y,
        },
        VIEW_SIZE,
    ));
}
