//! This examples compares Tonemapping options

use bevy::{
    asset::UnapprovedPathMode,
    core_pipeline::tonemapping::Tonemapping,
    pbr::CascadeShadowConfigBuilder,
    platform::collections::HashMap,
    prelude::*,
    reflect::TypePath,
    render::{
        render_resource::{AsBindGroup, ShaderRef},
        view::{ColorGrading, ColorGradingGlobal, ColorGradingSection, Hdr},
    },
};
use std::f32::consts::PI;

/// This example uses a shader source file from the assets subdirectory
const SHADER_ASSET_PATH: &str = "shaders/tonemapping_test_patterns.wgsl";

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(AssetPlugin {
                // We enable loading assets from arbitrary filesystem paths as this example allows
                // drag and dropping a local image for color grading
                unapproved_path_mode: UnapprovedPathMode::Allow,
                ..default()
            }),
            MaterialPlugin::<ColorGradientMaterial>::default(),
        ))
        .insert_resource(CameraTransform(
            Transform::from_xyz(0.7, 0.7, 1.0).looking_at(Vec3::new(0.0, 0.3, 0.0), Vec3::Y),
        ))
        .init_resource::<PerMethodSettings>()
        .insert_resource(CurrentScene(1))
        .insert_resource(SelectedParameter { value: 0, max: 4 })
        .add_systems(
            Startup,
            (
                setup,
                setup_basic_scene,
                setup_color_gradient_scene,
                setup_image_viewer_scene,
            ),
        )
        .add_systems(
            Update,
            (
                drag_drop_image,
                resize_image,
                toggle_scene,
                toggle_tonemapping_method,
                update_color_grading_settings,
                update_ui,
            ),
        )
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    camera_transform: Res<CameraTransform>,
) {
    // camera
    commands.spawn((
        Camera3d::default(),
        Hdr,
        camera_transform.0,
        DistanceFog {
            color: Color::srgb_u8(43, 44, 47),
            falloff: FogFalloff::Linear {
                start: 1.0,
                end: 8.0,
            },
            ..default()
        },
        EnvironmentMapLight {
            diffuse_map: asset_server.load("environment_maps/pisa_diffuse_rgb9e5_zstd.ktx2"),
            specular_map: asset_server.load("environment_maps/pisa_specular_rgb9e5_zstd.ktx2"),
            intensity: 2000.0,
            ..default()
        },
    ));

    // ui
    commands.spawn((
        Text::default(),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        },
    ));
}

fn setup_basic_scene(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Main scene
    commands.spawn((
        SceneRoot(asset_server.load(
            GltfAssetLabel::Scene(0).from_asset("models/TonemappingTest/TonemappingTest.gltf"),
        )),
        SceneNumber(1),
    ));

    // Flight Helmet
    commands.spawn((
        SceneRoot(
            asset_server
                .load(GltfAssetLabel::Scene(0).from_asset("models/FlightHelmet/FlightHelmet.gltf")),
        ),
        Transform::from_xyz(0.5, 0.0, -0.5).with_rotation(Quat::from_rotation_y(-0.15 * PI)),
        SceneNumber(1),
    ));

    // light
    commands.spawn((
        DirectionalLight {
            illuminance: 15_000.,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::ZYX, 0.0, PI * -0.15, PI * -0.15)),
        CascadeShadowConfigBuilder {
            maximum_distance: 3.0,
            first_cascade_far_bound: 0.9,
            ..default()
        }
        .build(),
        SceneNumber(1),
    ));
}

fn setup_color_gradient_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorGradientMaterial>>,
    camera_transform: Res<CameraTransform>,
) {
    let mut transform = camera_transform.0;
    transform.translation += *transform.forward();

    commands.spawn((
        Mesh3d(meshes.add(Rectangle::new(0.7, 0.7))),
        MeshMaterial3d(materials.add(ColorGradientMaterial {})),
        transform,
        Visibility::Hidden,
        SceneNumber(2),
    ));
}

fn setup_image_viewer_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    camera_transform: Res<CameraTransform>,
) {
    let mut transform = camera_transform.0;
    transform.translation += *transform.forward();

    // exr/hdr viewer (exr requires enabling bevy feature)
    commands.spawn((
        Mesh3d(meshes.add(Rectangle::default())),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: None,
            unlit: true,
            ..default()
        })),
        transform,
        Visibility::Hidden,
        SceneNumber(3),
        HDRViewer,
    ));

    commands.spawn((
        Text::new("Drag and drop an HDR or EXR file"),
        TextFont {
            font_size: 36.0,
            ..default()
        },
        TextColor(Color::BLACK),
        TextLayout::new_with_justify(Justify::Center),
        Node {
            align_self: AlignSelf::Center,
            margin: UiRect::all(Val::Auto),
            ..default()
        },
        SceneNumber(3),
        Visibility::Hidden,
    ));
}

// ----------------------------------------------------------------------------

fn drag_drop_image(
    image_mat: Query<&MeshMaterial3d<StandardMaterial>, With<HDRViewer>>,
    text: Query<Entity, (With<Text>, With<SceneNumber>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut drop_events: EventReader<FileDragAndDrop>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    let Some(new_image) = drop_events.read().find_map(|e| match e {
        FileDragAndDrop::DroppedFile { path_buf, .. } => {
            Some(asset_server.load(path_buf.to_string_lossy().to_string()))
        }
        _ => None,
    }) else {
        return;
    };

    for mat_h in &image_mat {
        if let Some(mat) = materials.get_mut(mat_h) {
            mat.base_color_texture = Some(new_image.clone());

            // Despawn the image viewer instructions
            if let Ok(text_entity) = text.single() {
                commands.entity(text_entity).despawn();
            }
        }
    }
}

fn resize_image(
    image_mesh: Query<(&MeshMaterial3d<StandardMaterial>, &Mesh3d), With<HDRViewer>>,
    materials: Res<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    images: Res<Assets<Image>>,
    mut image_events: EventReader<AssetEvent<Image>>,
) {
    for event in image_events.read() {
        let (AssetEvent::Added { id } | AssetEvent::Modified { id }) = event else {
            continue;
        };

        for (mat_h, mesh_h) in &image_mesh {
            let Some(mat) = materials.get(mat_h) else {
                continue;
            };

            let Some(ref base_color_texture) = mat.base_color_texture else {
                continue;
            };

            if *id != base_color_texture.id() {
                continue;
            };

            let Some(image_changed) = images.get(*id) else {
                continue;
            };

            let size = image_changed.size_f32().normalize_or_zero() * 1.4;
            // Resize Mesh
            let quad = Mesh::from(Rectangle::from_size(size));
            meshes.insert(mesh_h, quad);
        }
    }
}

fn toggle_scene(
    keys: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&mut Visibility, &SceneNumber)>,
    mut current_scene: ResMut<CurrentScene>,
) {
    let mut pressed = None;
    if keys.just_pressed(KeyCode::KeyQ) {
        pressed = Some(1);
    } else if keys.just_pressed(KeyCode::KeyW) {
        pressed = Some(2);
    } else if keys.just_pressed(KeyCode::KeyE) {
        pressed = Some(3);
    }

    if let Some(pressed) = pressed {
        current_scene.0 = pressed;

        for (mut visibility, scene) in query.iter_mut() {
            if scene.0 == pressed {
                *visibility = Visibility::Visible;
            } else {
                *visibility = Visibility::Hidden;
            }
        }
    }
}

fn toggle_tonemapping_method(
    keys: Res<ButtonInput<KeyCode>>,
    mut tonemapping: Single<&mut Tonemapping>,
    mut color_grading: Single<&mut ColorGrading>,
    per_method_settings: Res<PerMethodSettings>,
) {
    if keys.just_pressed(KeyCode::Digit1) {
        **tonemapping = Tonemapping::None;
    } else if keys.just_pressed(KeyCode::Digit2) {
        **tonemapping = Tonemapping::Reinhard;
    } else if keys.just_pressed(KeyCode::Digit3) {
        **tonemapping = Tonemapping::ReinhardLuminance;
    } else if keys.just_pressed(KeyCode::Digit4) {
        **tonemapping = Tonemapping::AcesFitted;
    } else if keys.just_pressed(KeyCode::Digit5) {
        **tonemapping = Tonemapping::AgX;
    } else if keys.just_pressed(KeyCode::Digit6) {
        **tonemapping = Tonemapping::SomewhatBoringDisplayTransform;
    } else if keys.just_pressed(KeyCode::Digit7) {
        **tonemapping = Tonemapping::TonyMcMapface;
    } else if keys.just_pressed(KeyCode::Digit8) {
        **tonemapping = Tonemapping::BlenderFilmic;
    }

    **color_grading = (*per_method_settings
        .settings
        .get::<Tonemapping>(&tonemapping)
        .as_ref()
        .unwrap())
    .clone();
}

#[derive(Resource)]
struct SelectedParameter {
    value: i32,
    max: i32,
}

impl SelectedParameter {
    fn next(&mut self) {
        self.value = (self.value + 1).rem_euclid(self.max);
    }
    fn prev(&mut self) {
        self.value = (self.value - 1).rem_euclid(self.max);
    }
}

fn update_color_grading_settings(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut per_method_settings: ResMut<PerMethodSettings>,
    tonemapping: Single<&Tonemapping>,
    current_scene: Res<CurrentScene>,
    mut selected_parameter: ResMut<SelectedParameter>,
) {
    let color_grading = per_method_settings.settings.get_mut(*tonemapping).unwrap();
    let mut dt = time.delta_secs() * 0.25;
    if keys.pressed(KeyCode::ArrowLeft) {
        dt = -dt;
    }

    if keys.just_pressed(KeyCode::ArrowDown) {
        selected_parameter.next();
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        selected_parameter.prev();
    }
    if keys.pressed(KeyCode::ArrowLeft) || keys.pressed(KeyCode::ArrowRight) {
        match selected_parameter.value {
            0 => {
                color_grading.global.exposure += dt;
            }
            1 => {
                color_grading
                    .all_sections_mut()
                    .for_each(|section| section.gamma += dt);
            }
            2 => {
                color_grading
                    .all_sections_mut()
                    .for_each(|section| section.saturation += dt);
            }
            3 => {
                color_grading.global.post_saturation += dt;
            }
            _ => {}
        }
    }

    if keys.just_pressed(KeyCode::Space) {
        for (_, grading) in per_method_settings.settings.iter_mut() {
            *grading = ColorGrading::default();
        }
    }

    if keys.just_pressed(KeyCode::Enter) && current_scene.0 == 1 {
        for (mapper, grading) in per_method_settings.settings.iter_mut() {
            *grading = PerMethodSettings::basic_scene_recommendation(*mapper);
        }
    }
}

fn update_ui(
    mut text_query: Single<&mut Text, Without<SceneNumber>>,
    settings: Single<(&Tonemapping, &ColorGrading)>,
    current_scene: Res<CurrentScene>,
    selected_parameter: Res<SelectedParameter>,
    mut hide_ui: Local<bool>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if keys.just_pressed(KeyCode::KeyH) {
        *hide_ui = !*hide_ui;
    }

    if *hide_ui {
        if !text_query.is_empty() {
            // single_mut() always triggers change detection,
            // so only access if text actually needs changing
            text_query.clear();
        }
        return;
    }

    let (tonemapping, color_grading) = *settings;
    let tonemapping = *tonemapping;

    let mut text = String::with_capacity(text_query.len());

    let scn = current_scene.0;
    text.push_str("(H) Hide UI\n\n");
    text.push_str("Test Scene: \n");
    text.push_str(&format!(
        "(Q) {} Basic Scene\n",
        if scn == 1 { ">" } else { "" }
    ));
    text.push_str(&format!(
        "(W) {} Color Sweep\n",
        if scn == 2 { ">" } else { "" }
    ));
    text.push_str(&format!(
        "(E) {} Image Viewer\n",
        if scn == 3 { ">" } else { "" }
    ));

    text.push_str("\n\nTonemapping Method:\n");
    text.push_str(&format!(
        "(1) {} Disabled\n",
        if tonemapping == Tonemapping::None {
            ">"
        } else {
            ""
        }
    ));
    text.push_str(&format!(
        "(2) {} Reinhard\n",
        if tonemapping == Tonemapping::Reinhard {
            "> "
        } else {
            ""
        }
    ));
    text.push_str(&format!(
        "(3) {} Reinhard Luminance\n",
        if tonemapping == Tonemapping::ReinhardLuminance {
            ">"
        } else {
            ""
        }
    ));
    text.push_str(&format!(
        "(4) {} ACES Fitted\n",
        if tonemapping == Tonemapping::AcesFitted {
            ">"
        } else {
            ""
        }
    ));
    text.push_str(&format!(
        "(5) {} AgX\n",
        if tonemapping == Tonemapping::AgX {
            ">"
        } else {
            ""
        }
    ));
    text.push_str(&format!(
        "(6) {} SomewhatBoringDisplayTransform\n",
        if tonemapping == Tonemapping::SomewhatBoringDisplayTransform {
            ">"
        } else {
            ""
        }
    ));
    text.push_str(&format!(
        "(7) {} TonyMcMapface\n",
        if tonemapping == Tonemapping::TonyMcMapface {
            ">"
        } else {
            ""
        }
    ));
    text.push_str(&format!(
        "(8) {} Blender Filmic\n",
        if tonemapping == Tonemapping::BlenderFilmic {
            ">"
        } else {
            ""
        }
    ));

    text.push_str("\n\nColor Grading:\n");
    text.push_str("(arrow keys)\n");
    if selected_parameter.value == 0 {
        text.push_str("> ");
    }
    text.push_str(&format!("Exposure: {}\n", color_grading.global.exposure));
    if selected_parameter.value == 1 {
        text.push_str("> ");
    }
    text.push_str(&format!("Gamma: {}\n", color_grading.shadows.gamma));
    if selected_parameter.value == 2 {
        text.push_str("> ");
    }
    text.push_str(&format!(
        "PreSaturation: {}\n",
        color_grading.shadows.saturation
    ));
    if selected_parameter.value == 3 {
        text.push_str("> ");
    }
    text.push_str(&format!(
        "PostSaturation: {}\n",
        color_grading.global.post_saturation
    ));
    text.push_str("(Space) Reset all to default\n");

    if current_scene.0 == 1 {
        text.push_str("(Enter) Reset all to scene recommendation\n");
    }

    if text != text_query.as_str() {
        // single_mut() always triggers change detection,
        // so only access if text actually changed
        text_query.0 = text;
    }
}

// ----------------------------------------------------------------------------

#[derive(Resource)]
struct PerMethodSettings {
    settings: HashMap<Tonemapping, ColorGrading>,
}

impl PerMethodSettings {
    fn basic_scene_recommendation(method: Tonemapping) -> ColorGrading {
        match method {
            Tonemapping::Reinhard | Tonemapping::ReinhardLuminance => ColorGrading {
                global: ColorGradingGlobal {
                    exposure: 0.5,
                    ..default()
                },
                ..default()
            },
            Tonemapping::AcesFitted => ColorGrading {
                global: ColorGradingGlobal {
                    exposure: 0.35,
                    ..default()
                },
                ..default()
            },
            Tonemapping::AgX => ColorGrading::with_identical_sections(
                ColorGradingGlobal {
                    exposure: -0.2,
                    post_saturation: 1.1,
                    ..default()
                },
                ColorGradingSection {
                    saturation: 1.1,
                    ..default()
                },
            ),
            _ => ColorGrading::default(),
        }
    }
}

impl Default for PerMethodSettings {
    fn default() -> Self {
        let mut settings = <HashMap<_, _>>::default();

        for method in [
            Tonemapping::None,
            Tonemapping::Reinhard,
            Tonemapping::ReinhardLuminance,
            Tonemapping::AcesFitted,
            Tonemapping::AgX,
            Tonemapping::SomewhatBoringDisplayTransform,
            Tonemapping::TonyMcMapface,
            Tonemapping::BlenderFilmic,
        ] {
            settings.insert(
                method,
                PerMethodSettings::basic_scene_recommendation(method),
            );
        }

        Self { settings }
    }
}

impl Material for ColorGradientMaterial {
    fn fragment_shader() -> ShaderRef {
        SHADER_ASSET_PATH.into()
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct ColorGradientMaterial {}

#[derive(Resource)]
struct CameraTransform(Transform);

#[derive(Resource)]
struct CurrentScene(u32);

#[derive(Component)]
struct SceneNumber(u32);

#[derive(Component)]
struct HDRViewer;
