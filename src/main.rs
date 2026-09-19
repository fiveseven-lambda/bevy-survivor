use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use parry2d::bounding_volume::Aabb;
use parry2d::partitioning::{Bvh, BvhWorkspace};
use rand::RngExt;

const VIEWPORT_RATIO: UVec2 = uvec2(16, 9);
const VIEW_SIZE: Vec2 = vec2(64.0, 36.0);
const FIELD_SIZE: UVec2 = uvec2(16 * 16, 16 * 9);
const FIELD_WIDTH: usize = FIELD_SIZE.x as usize;
const FIELD_HEIGHT: usize = FIELD_SIZE.y as usize;

const RESOURCE_COLORS: [Color; 3] = [
    Color::hsl(0.0, 0.8, 0.5),
    Color::hsl(120.0, 0.8, 0.5),
    Color::hsl(240.0, 0.8, 0.5),
];

#[derive(Resource)]
struct Field(Vec<Vec<u8>>);

#[derive(Resource)]
struct Player {
    position: Vec2,
    direction: Vec2,
    resources: [f32; 3],
}

#[derive(Resource)]
struct ResourceColors([Handle<ColorMaterial>; 3]);

#[derive(Component)]
struct PlayerStroke;

#[derive(Component)]
struct PlayerFill;

#[derive(Component)]
struct Approaching {
    speed: f32,
}

#[derive(Resource)]
struct EnemySource {
    stroke: Handle<Mesh>,
    stroke_color: Handle<ColorMaterial>,
    tick: std::time::Duration,
}

#[derive(Resource)]
struct Enemies {
    bvh: Bvh,
    workspace: BvhWorkspace,
    count: usize,
}

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
        .add_systems(Update, move_enemies_approaching)
        .add_systems(Update, create_enemies_approaching)
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
    for _ in 0..100 {
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
    let resource_colors = RESOURCE_COLORS.map(|color| materials.add(color));
    let player_fill_color = resource_colors[field[0][0] as usize].clone();
    commands.spawn((
        PlayerFill,
        Mesh2d(meshes.add(player_fill)),
        MeshMaterial2d(player_fill_color),
    ));
    commands.insert_resource(ResourceColors(resource_colors));

    let player_stroke = Circle::new(1.0).to_ring(0.1);
    let player_stroke_color = Color::WHITE;
    let index = commands
        .spawn((
            PlayerStroke,
            Mesh2d(meshes.add(player_stroke)),
            MeshMaterial2d(materials.add(player_stroke_color)),
        ))
        .id()
        .index_u32();
    let mut bvh = Bvh::new();
    bvh.insert(Aabb::new(-Vec2::splat(5.0), Vec2::splat(5.0)), index);

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

    let enemy_stroke = Circle::new(0.5).to_ring(0.1);
    let enemy_stroke_color = Color::BLACK;
    commands.insert_resource(EnemySource {
        stroke: meshes.add(enemy_stroke),
        stroke_color: materials.add(enemy_stroke_color),
        tick: std::time::Duration::ZERO,
    });

    commands.insert_resource(Enemies {
        bvh,
        workspace: BvhWorkspace::default(),
        count: 0,
    });
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
    mut background: Single<&mut Sprite>,
    mut player_fill: Single<&mut MeshMaterial2d<ColorMaterial>, With<PlayerFill>>,
    resource_colors: Res<ResourceColors>,
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
    player_fill.0 = resource_colors.0[cell].clone();
    background.rect = Some(Rect::from_center_size(
        Vec2 {
            x: player.position.x,
            y: -player.position.y,
        },
        VIEW_SIZE,
    ));
}

fn create_enemies_approaching(
    mut commands: Commands,
    mut enemy_source: ResMut<EnemySource>,
    mut enemies: ResMut<Enemies>,
    time: Res<Time>,
) {
    let mut rng = rand::rng();
    enemy_source.tick += time.delta();
    let t = std::time::Duration::from_millis(10);
    while enemy_source.tick > t {
        enemy_source.tick -= t;
        let pos = 40.0 * Vec2::from_angle(rng.random_range(0.0..360.0));
        let index = commands
            .spawn((
                Mesh2d(enemy_source.stroke.clone()),
                MeshMaterial2d(enemy_source.stroke_color.clone()),
                Approaching { speed: 10.0 },
                Transform::from_translation(pos.extend(0.0)),
            ))
            .id()
            .index_u32();
        enemies.bvh.insert(
            Aabb::new(pos - Vec2::splat(5.0), pos + Vec2::splat(5.0)),
            index,
        );
    }
}

fn move_enemies_approaching(
    mut commands: Commands,
    query: Query<(Entity, &mut Transform, &Approaching)>,
    enemies: ResMut<Enemies>,
    time: Res<Time>,
    player: ResMut<Player>,
    mut text: Single<&mut Text2d>,
) {
    let enemies = enemies.into_inner();
    enemies.bvh.refit(&mut enemies.workspace);
    enemies.count += 1;
    if enemies.count > 10 {
        enemies.bvh.optimize_incremental(&mut enemies.workspace);
        enemies.count = 0;
    }
    let mut count = 0;
    for (entity, mut enemy_transform, enemy) in query {
        let old_pos = enemy_transform.translation.truncate();
        let delta = -enemy.speed * time.delta_secs() * old_pos.normalize();
        let mut collision = Vec2::ZERO;
        let aabb = Aabb::new(old_pos - Vec2::splat(5.0), old_pos + Vec2::splat(5.0));
        for index in enemies.bvh.intersect_aabb(&aabb) {
            if index == entity.index_u32() {
                continue;
            }
            let diff = old_pos - enemies.bvh.leaf_node(index).unwrap().center();
            let distance = diff.length();
            if distance < 10.0 {
                collision += 0.1 * diff / distance.powf(2.0) * delta.length();
            }
        }
        let new_pos =
            old_pos - 10. * player.direction.normalize() * time.delta_secs() + delta + collision;
        if new_pos.length() > 40.0 || new_pos.length() < 1.0 {
            enemies.bvh.remove(entity.index_u32());
            commands.entity(entity).despawn();
        } else {
            count += 1;
            let aabb = Aabb::new(new_pos - Vec2::splat(5.0), new_pos + Vec2::splat(5.0));
            enemies.bvh.insert(aabb, entity.index_u32());
            enemy_transform.translation = new_pos.extend(0.0);
        }
    }
    text.0 = count.to_string();
}
