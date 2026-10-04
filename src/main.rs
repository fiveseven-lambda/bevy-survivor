use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use rand::RngExt;
use std::sync::{Arc, Mutex};

const VIEWPORT_RATIO: UVec2 = uvec2(16, 9);
const VIEW_SIZE: Vec2 = vec2(64.0, 36.0);
const FIELD_SIZE: UVec2 = uvec2(64 * 16, 64 * 9);
const FIELD_WIDTH: usize = FIELD_SIZE.x as usize;
const FIELD_HEIGHT: usize = FIELD_SIZE.y as usize;

const RESOURCE_COLORS: [Color; 3] = [
    Color::hsv(120.0, 0.6, 0.8),
    Color::hsv(210.0, 0.6, 0.8),
    Color::hsv(340.0, 0.6, 0.8),
];

#[derive(Clone, PartialEq, Eq, Hash, Debug, States)]
enum State {
    Loading,
    Paused,
    Running,
}

#[derive(Component)]
struct Loading;

#[derive(Resource)]
struct LoadingState(Arc<Mutex<Option<LoadingMessage>>>);

enum LoadingMessage {
    Loading(f32),
    Ready(Vec<Vec<u8>>),
}

#[derive(Component)]
struct Paused;

#[derive(Component)]
struct ProgressBar;

#[derive(Resource)]
struct Field(Vec<Vec<u8>>);

#[derive(Resource)]
struct PlayerDirection(Vec2);

#[derive(Resource)]
struct Player {
    position: Vec2,
    colors: [Handle<ColorMaterial>; 3],
}

#[derive(Component)]
struct PlayerStroke;

#[derive(Component)]
struct PlayerFill;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            update_viewport.run_if(on_message::<bevy::window::WindowResized>),
        )
        .add_systems(
            Update,
            update_loading_screen.run_if(in_state(State::Loading)),
        )
        .add_systems(OnEnter(State::Paused), setup_title_screen)
        .add_systems(Update, resume.run_if(in_state(State::Paused)))
        .add_systems(
            Update,
            update_direction.run_if(in_state(State::Running).and_then(on_message::<CursorMoved>)),
        )
        .add_systems(Update, update.run_if(in_state(State::Running)))
        .insert_state(State::Loading)
        .run();
}

fn setup(
    mut commands: Commands,
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

    let loading_state = Arc::new(Mutex::new(Some(LoadingMessage::Loading(0.0))));
    commands.insert_resource(LoadingState(loading_state.clone()));
    std::thread::spawn(move || {
        let mut rng = rand::rng();
        let mut field_values: Vec<Vec<[f64; 3]>> = (0..FIELD_HEIGHT)
            .map(|_| (0..FIELD_WIDTH).map(|_| rng.random()).collect())
            .collect();
        const NUM_STEPS: u32 = 1000;
        for step in 0..NUM_STEPS {
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
            *loading_state.lock().unwrap() =
                Some(LoadingMessage::Loading(step as f32 / NUM_STEPS as f32));
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
        *loading_state.lock().unwrap() = Some(LoadingMessage::Ready(field));
    });

    let progress_bar = Rectangle::from_size(Vec2::new(56.0, 4.0));
    let progress_bar_border = progress_bar.to_ring(0.1);
    let white = materials.add(Color::WHITE);
    commands.spawn((
        ProgressBar,
        Loading,
        Mesh2d(meshes.add(progress_bar)),
        MeshMaterial2d(white.clone()),
        Transform {
            translation: Vec3 {
                x: -28.0,
                y: 0.0,
                z: 0.0,
            },
            rotation: Quat::IDENTITY,
            scale: Vec3 {
                x: 0.0,
                y: 1.0,
                z: 1.0,
            },
        },
    ));
    commands.spawn((
        Loading,
        Mesh2d(meshes.add(progress_bar_border)),
        MeshMaterial2d(white.clone()),
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

fn update_loading_screen(
    mut commands: Commands,
    mut progress_bar: Single<&mut Transform, With<ProgressBar>>,
    loading_state: Res<LoadingState>,
) {
    let loading_message = match loading_state.0.try_lock() {
        Ok(ref mut message) => message.take(),
        Err(_) => return,
    };
    let Some(loading_message) = loading_message else {
        return;
    };
    match loading_message {
        LoadingMessage::Loading(progress) => {
            progress_bar.translation.x = 28.0 * (progress - 1.0);
            progress_bar.scale.x = progress;
        }
        LoadingMessage::Ready(field) => {
            commands.set_state(State::Paused);
            commands.insert_resource(Field(field));
        }
    }
}

fn setup_title_screen(
    mut commands: Commands,
    loading: Query<Entity, With<Loading>>,
    field: Res<Field>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for entity in loading {
        commands.entity(entity).despawn();
    }
    let mut field_image_data = Vec::new();
    for row in field.0.iter().rev() {
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
    let colors = RESOURCE_COLORS.map(|color| materials.add(color));
    let player_fill_color = colors[field.0[0][0] as usize].clone();
    commands.spawn((
        PlayerFill,
        Mesh2d(meshes.add(player_fill)),
        MeshMaterial2d(player_fill_color),
    ));
    commands.insert_resource(Player {
        position: Vec2::ZERO,
        colors,
    });

    let player_stroke = Circle::new(1.0).to_ring(0.1);
    let player_stroke_color = Color::WHITE;
    commands.spawn((
        PlayerStroke,
        Mesh2d(meshes.add(player_stroke)),
        MeshMaterial2d(materials.add(player_stroke_color)),
    ));

    commands.spawn((
        Paused,
        Mesh2d(meshes.add(Rectangle::from_size(VIEW_SIZE))),
        MeshMaterial2d(materials.add(Color::hsva(0.0, 0.0, 0.0, 0.9))),
    ));
    commands.spawn((
        Paused,
        Text2d(String::from("Press Space")),
        TextFont {
            font_size: FontSize::from(80.0),
            ..default()
        },
        Transform::from_scale(Vec3 {
            x: 0.1,
            y: 0.1,
            z: 1.0,
        }),
    ));
}

fn resume(
    mut commands: Commands,
    mut next_state: ResMut<NextState<State>>,
    paused: Query<Entity, With<Paused>>,
    mut keyboard_inputs: MessageReader<bevy::input::keyboard::KeyboardInput>,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform)>,
) {
    for keyboard_input in keyboard_inputs.read() {
        if keyboard_input.key_code == KeyCode::Space
            && keyboard_input.state == bevy::input::ButtonState::Released
        {
            for entity in paused {
                commands.entity(entity).despawn();
            }
            commands.insert_resource(PlayerDirection(calculate_direction(*window, *camera)));
            next_state.set(State::Running);
            return;
        }
    }
}

fn update_direction(
    mut player_direction: ResMut<PlayerDirection>,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform)>,
) {
    player_direction.0 = calculate_direction(*window, *camera);
}

fn calculate_direction(
    window: &Window,
    (camera, camera_transform): (&Camera, &GlobalTransform),
) -> Vec2 {
    if let Some(viewport_position) = window.cursor_position()
        && let Ok(world_position) = camera.viewport_to_world_2d(camera_transform, viewport_position)
    {
        world_position
    } else {
        Vec2::X
    }
}

fn update(
    mut player: ResMut<Player>,
    mut player_fill: Single<&mut MeshMaterial2d<ColorMaterial>, With<PlayerFill>>,
    mut background: Single<&mut Sprite>,
    player_direction: Res<PlayerDirection>,
    field: Res<Field>,
    time: Res<Time>,
) {
    let pos_float =
        player.position + 10. * player_direction.0.normalize_or(Vec2::X) * time.delta_secs();
    let pos_int = pos_float.as_ivec2();
    let field_size = FIELD_SIZE.as_ivec2();
    let q = pos_int.div_euclid(field_size) * field_size;
    player.position = pos_float - q.as_vec2();
    let IVec2 { x: column, y: row } = pos_int - q;
    let cell = field.0[row as usize][column as usize] as usize;
    player_fill.0 = player.colors[cell].clone();
    background.rect = Some(Rect::from_center_size(
        Vec2 {
            x: player.position.x,
            y: -player.position.y,
        },
        VIEW_SIZE,
    ));
}
