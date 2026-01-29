use crate::{
    batching::gpu_preprocessing::{GpuPreprocessingMode, GpuPreprocessingSupport},
    // GPU 预处理模式和支持
    extract_component::{ExtractComponent, ExtractComponentPlugin},
    // 组件提取插件
    extract_resource::{ExtractResource, ExtractResourcePlugin},
    // 资源提取插件
    render_asset::RenderAssets,
    render_resource::TextureView,
    // 纹理视图
    sync_world::{RenderEntity, SyncToRenderWorld},
    // 同步到渲染世界
    texture::{GpuImage, ManualTextureViews},
    // GPU 图像和手动纹理视图
    view::{
        ColorGrading, ExtractedView, ExtractedWindows, Msaa, NoIndirectDrawing,
        RenderVisibleEntities, RetainedViewEntity, ViewUniformOffset,
    },
    // 视图相关组件
    Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
};

use bevy_app::{App, Plugin, PostStartup, PostUpdate};
use bevy_asset::{AssetEvent, AssetEventSystems, AssetId, Assets};
use bevy_camera::{
    primitives::Frustum,
    // 视锥体
    visibility::{self, RenderLayers, VisibleEntities},
    // 可见性系统
    Camera, Camera2d, Camera3d, CameraMainTextureUsages, CameraOutputMode, CameraUpdateSystems,
    ClearColor, ClearColorConfig, Exposure, Hdr, ManualTextureViewHandle, MsaaWriteback,
    NormalizedRenderTarget, Projection, RenderTarget, RenderTargetInfo, Viewport,
    // 渲染目标和视口
};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::{
    change_detection::DetectChanges,
    // 变更检测
    component::Component,
    // 组件
    entity::{ContainsEntity, Entity},
    // 实体
    error::BevyError,
    // 错误类型
    lifecycle::HookContext,
    // 生命周期钩子上下文
    message::MessageReader,
    // 消息读取器
    prelude::With,
    // 查询过滤器
    query::{Has, QueryItem},
    // 查询相关
    reflect::ReflectComponent,
    // 反射组件
    resource::Resource,
    schedule::{InternedScheduleLabel, IntoScheduleConfigs, ScheduleLabel},
    system::{Commands, Query, Res, ResMut},
    // 系统参数
    world::DeferredWorld,
    // 延迟世界
};
use bevy_image::Image;
use bevy_log::warn;
use bevy_log::warn_once;
use bevy_math::{uvec2, vec2, Mat4, URect, UVec2, UVec4, Vec2};
// 数学类型
use bevy_platform::collections::{HashMap, HashSet};
// 集合类型
use bevy_reflect::prelude::*;
// 反射
use bevy_transform::components::GlobalTransform;
// 全局变换
use bevy_window::{PrimaryWindow, Window, WindowCreated, WindowResized, WindowScaleFactorChanged};
use wgpu::TextureFormat;
// WGPU 纹理格式

/// 相机插件 - 负责相机系统的初始化和管理
#[derive(Default)]
pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.register_required_components::<Camera, Msaa>()
            // 注册相机必需的 Msaa 组件
            .register_required_components::<Camera, SyncToRenderWorld>()
            // 注册相机必需的 SyncToRenderWorld 组件
            .register_required_components::<Camera3d, ColorGrading>()
            // 注册 3D 相机必需的 ColorGrading 组件
            .register_required_components::<Camera3d, Exposure>()
            // 注册 3D 相机必需的 Exposure 组件
            .add_plugins((
                ExtractResourcePlugin::<ClearColor>::default(),
                // 添加清除颜色提取插件
                ExtractComponentPlugin::<CameraMainTextureUsages>::default(),
                // 添加相机主纹理用法提取插件
            ))
            .add_systems(PostStartup, camera_system.in_set(CameraUpdateSystems))
            // 在启动后添加相机系统
            .add_systems(
                PostUpdate,
                camera_system
                    .in_set(CameraUpdateSystems)
                    .before(AssetEventSystems)
                    .before(visibility::update_frusta),
            );
            // 在更新后添加相机系统
        app.world_mut()
            .register_component_hooks::<Camera>()
            .on_add(warn_on_no_render_graph);
            // 注册相机组件的添加钩子

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .init_resource::<SortedCameras>()
                // 初始化排序相机资源
                .add_systems(ExtractSchedule, extract_cameras)
                // 添加相机提取系统
                .add_systems(Render, sort_cameras.in_set(RenderSystems::ManageViews));
        }
    }
}

/// 当相机没有配置渲染图时发出警告
fn warn_on_no_render_graph(world: DeferredWorld, HookContext { entity, caller, .. }: HookContext) {
    if !world.entity(entity).contains::<CameraRenderGraph>() {
        warn!("{}Entity {entity} has a `Camera` component, but it doesn't have a render graph configured. Usually, adding a `Camera2d` or `Camera3d` component will work.
        However, you may instead need to enable `bevy_core_pipeline`, or may want to manually add a `CameraRenderGraph` component to create a custom render graph.", caller.map(|location|format!("{location}: ")).unwrap_or_default());
    }
}

/// ClearColor 的资源提取实现
impl ExtractResource for ClearColor {
    type Source = Self;

    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}
/// CameraMainTextureUsages 的组件提取实现
impl ExtractComponent for CameraMainTextureUsages {
    type QueryData = &'static Self;
    type QueryFilter = ();
    type Out = Self;

    fn extract_component(item: QueryItem<Self::QueryData>) -> Option<Self::Out> {
        Some(*item)
    }
}
impl ExtractComponent for Camera2d {
    type QueryData = &'static Self;
    type QueryFilter = With<Camera>;
    type Out = Self;

    fn extract_component(item: QueryItem<Self::QueryData>) -> Option<Self::Out> {
        Some(item.clone())
    }
}
/// Camera3d 的组件提取实现
impl ExtractComponent for Camera3d {
    type QueryData = &'static Self;
    type QueryFilter = With<Camera>;
    type Out = Self;

    fn extract_component(item: QueryItem<Self::QueryData>) -> Option<Self::Out> {
        Some(item.clone())
    }
}

/// Configures the render schedule to be run for a given [`Camera`] entity.
#[derive(Component, Debug, Deref, DerefMut, Reflect, Clone)]
#[reflect(opaque)]
#[reflect(Component, Debug, Clone)]
pub struct CameraRenderGraph(pub InternedScheduleLabel);

impl CameraRenderGraph {
    /// Creates a new [`CameraRenderGraph`] from a schedule label.
    #[inline]
    pub fn new<T: ScheduleLabel>(schedule: T) -> Self {
        Self(schedule.intern())
    }

    /// Sets the schedule.
    #[inline]
    pub fn set<T: ScheduleLabel>(&mut self, schedule: T) {
        self.0 = schedule.intern();
    }
}

/// 规范化渲染目标的扩展特性
pub trait NormalizedRenderTargetExt {
    fn get_texture_view<'a>(
        &self,
        windows: &'a ExtractedWindows,
        images: &'a RenderAssets<GpuImage>,
        manual_texture_views: &'a ManualTextureViews,
    ) -> Option<&'a TextureView>;

    /// Retrieves the [`TextureFormat`] of this render target, if it exists.
    fn get_texture_view_format<'a>(
        &self,
        windows: &'a ExtractedWindows,
        images: &'a RenderAssets<GpuImage>,
        manual_texture_views: &'a ManualTextureViews,
    ) -> Option<TextureFormat>;

    fn get_render_target_info<'a>(
        &self,
        resolutions: impl IntoIterator<Item = (Entity, &'a Window)>,
        images: &Assets<Image>,
        manual_texture_views: &ManualTextureViews,
    ) -> Result<RenderTargetInfo, MissingRenderTargetInfoError>;

    // Check if this render target is contained in the given changed windows or images.
    fn is_changed(
        &self,
        changed_window_ids: &HashSet<Entity>,
        changed_image_handles: &HashSet<&AssetId<Image>>,
    ) -> bool;
}

impl NormalizedRenderTargetExt for NormalizedRenderTarget {
    fn get_texture_view<'a>(
        &self,
        windows: &'a ExtractedWindows,
        images: &'a RenderAssets<GpuImage>,
        manual_texture_views: &'a ManualTextureViews,
    ) -> Option<&'a TextureView> {
        match self {
            NormalizedRenderTarget::Window(window_ref) => windows
                .get(&window_ref.entity())
                .and_then(|window| window.swap_chain_texture_view.as_ref()),
            NormalizedRenderTarget::Image(image_target) => images
                .get(&image_target.handle)
                .map(|image| &image.texture_view),
            NormalizedRenderTarget::TextureView(id) => {
                manual_texture_views.get(id).map(|tex| &tex.texture_view)
            }
            NormalizedRenderTarget::None { .. } => None,
        }
    }

    /// Retrieves the texture view's [`TextureFormat`] of this render target, if it exists.
    fn get_texture_view_format<'a>(
        &self,
        windows: &'a ExtractedWindows,
        images: &'a RenderAssets<GpuImage>,
        manual_texture_views: &'a ManualTextureViews,
    ) -> Option<TextureFormat> {
        match self {
            NormalizedRenderTarget::Window(window_ref) => windows
                .get(&window_ref.entity())
                .and_then(|window| window.swap_chain_texture_view_format),
            NormalizedRenderTarget::Image(image_target) => {
                images.get(&image_target.handle).map(GpuImage::view_format)
            }
            NormalizedRenderTarget::TextureView(id) => {
                manual_texture_views.get(id).map(|tex| tex.view_format)
            }
            NormalizedRenderTarget::None { .. } => None,
        }
    }

    fn get_render_target_info<'a>(
        &self,
        resolutions: impl IntoIterator<Item = (Entity, &'a Window)>,
        images: &Assets<Image>,
        manual_texture_views: &ManualTextureViews,
    ) -> Result<RenderTargetInfo, MissingRenderTargetInfoError> {
        match self {
            NormalizedRenderTarget::Window(window_ref) => resolutions
                .into_iter()
                .find(|(entity, _)| *entity == window_ref.entity())
                .map(|(_, window)| RenderTargetInfo {
                    physical_size: window.physical_size(),
                    scale_factor: window.resolution.scale_factor(),
                })
                .ok_or(MissingRenderTargetInfoError::Window {
                    window: window_ref.entity(),
                }),
            NormalizedRenderTarget::Image(image_target) => images
                .get(&image_target.handle)
                .map(|image| RenderTargetInfo {
                    physical_size: image.size(),
                    scale_factor: image_target.scale_factor,
                })
                .ok_or(MissingRenderTargetInfoError::Image {
                    image: image_target.handle.id(),
                }),
            NormalizedRenderTarget::TextureView(id) => manual_texture_views
                .get(id)
                .map(|tex| RenderTargetInfo {
                    physical_size: tex.size,
                    scale_factor: 1.0,
                })
                .ok_or(MissingRenderTargetInfoError::TextureView { texture_view: *id }),
            NormalizedRenderTarget::None { width, height } => Ok(RenderTargetInfo {
                physical_size: uvec2(*width, *height),
                scale_factor: 1.0,
            }),
        }
    }

    // Check if this render target is contained in the given changed windows or images.
    fn is_changed(
        &self,
        changed_window_ids: &HashSet<Entity>,
        changed_image_handles: &HashSet<&AssetId<Image>>,
    ) -> bool {
        match self {
            NormalizedRenderTarget::Window(window_ref) => {
                changed_window_ids.contains(&window_ref.entity())
            }
            NormalizedRenderTarget::Image(image_target) => {
                changed_image_handles.contains(&image_target.handle.id())
            }
            NormalizedRenderTarget::TextureView(_) => true,
            NormalizedRenderTarget::None { .. } => false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MissingRenderTargetInfoError {
    #[error("RenderTarget::Window missing ({window:?}): Make sure the provided entity has a Window component.")]
    Window { window: Entity },
    #[error("RenderTarget::Image missing ({image:?}): Make sure the Image's usages include RenderAssetUsages::MAIN_WORLD.")]
    Image { image: AssetId<Image> },
    #[error("RenderTarget::TextureView missing ({texture_view:?}): make sure the texture view handle was not removed.")]
    TextureView {
        texture_view: ManualTextureViewHandle,
    },
}

/// System in charge of updating a [`Camera`] when its window or projection changes.
///
/// The system detects window creation, resize, and scale factor change events to update the camera
/// [`Projection`] if needed.
///
/// ## World Resources
///
/// [`Res<Assets<Image>>`](Assets<Image>) -- For cameras that render to an image, this resource is used to
/// inspect information about the render target. This system will not access any other image assets.
///
/// [`OrthographicProjection`]: bevy_camera::OrthographicProjection
/// [`PerspectiveProjection`]: bevy_camera::PerspectiveProjection
/// 相机系统 - 处理相机的更新和渲染目标信息
/// 
/// 该系统负责:
/// 1. 监听窗口和图像资产的变化
/// 2. 更新相机的渲染目标信息
/// 3. 调整视口大小以适应 DPI 变化
/// 4. 更新相机投影矩阵
pub fn camera_system(
    mut window_resized_reader: MessageReader<WindowResized>,
    mut window_created_reader: MessageReader<WindowCreated>,
    mut window_scale_factor_changed_reader: MessageReader<WindowScaleFactorChanged>,
    mut image_asset_event_reader: MessageReader<AssetEvent<Image>>,
    primary_window: Query<Entity, With<PrimaryWindow>>,
    windows: Query<(Entity, &Window)>,
    images: Res<Assets<Image>>,
    manual_texture_views: Res<ManualTextureViews>,
    mut cameras: Query<(&mut Camera, &RenderTarget, &mut Projection)>,
) -> Result<(), BevyError> {
    let primary_window = primary_window.iter().next();
    // 获取主窗口实体

    let mut changed_window_ids = <HashSet<_>>::default();
    changed_window_ids.extend(window_created_reader.read().map(|event| event.window));
    changed_window_ids.extend(window_resized_reader.read().map(|event| event.window));
    let scale_factor_changed_window_ids: HashSet<_> = window_scale_factor_changed_reader
        .read()
        .map(|event| event.window)
        .collect();
    changed_window_ids.extend(scale_factor_changed_window_ids.clone());
    // 收集所有发生变化的窗口 ID

    let changed_image_handles: HashSet<&AssetId<Image>> = image_asset_event_reader
        .read()
        .filter_map(|event| match event {
            AssetEvent::Modified { id } | AssetEvent::Added { id } => Some(id),
            _ => None,
        })
        .collect();
    // 收集所有发生变化的图像资产

    for (mut camera, render_target, mut camera_projection) in &mut cameras {
        let mut viewport_size = camera
            .viewport
            .as_ref()
            .map(|viewport| viewport.physical_size);
        // 获取视口的物理大小

        if let Some(normalized_target) = render_target.normalize(primary_window)
            && (normalized_target.is_changed(&changed_window_ids, &changed_image_handles)
                || camera.is_added()
                || camera_projection.is_changed()
                || camera.computed.old_viewport_size != viewport_size
                || camera.computed.old_sub_camera_view != camera.sub_camera_view)
        {
            let new_computed_target_info = normalized_target.get_render_target_info(
                windows,
                &images,
                &manual_texture_views,
            )?;
            // 获取新的渲染目标信息
            // Check for the scale factor changing, and resize the viewport if needed.
            // This can happen when the window is moved between monitors with different DPIs.
            // Without this, the viewport will take a smaller portion of the window moved to
            // a higher DPI monitor.
            if normalized_target.is_changed(&scale_factor_changed_window_ids, &HashSet::default())
                && let Some(old_scale_factor) = camera
                    .computed
                    .target_info
                    .as_ref()
                    .map(|info| info.scale_factor)
            {
                let resize_factor = new_computed_target_info.scale_factor / old_scale_factor;
                if let Some(ref mut viewport) = camera.viewport {
                    let resize = |vec: UVec2| (vec.as_vec2() * resize_factor).as_uvec2();
                    viewport.physical_position = resize(viewport.physical_position);
                    viewport.physical_size = resize(viewport.physical_size);
                    viewport_size = Some(viewport.physical_size);
                }
            }
            // 检查缩放因子变化,并在需要时调整视口大小
            // 这可能发生在窗口在具有不同 DPI 的显示器之间移动时
            // 如果没有此调整,视口在移动到高 DPI 显示器时会占据较小的窗口部分
            // This check is needed because when changing WindowMode to Fullscreen, the viewport may have invalid
            // arguments due to a sudden change on the window size to a lower value.
            // If the size of the window is lower, the viewport will match that lower value.
            if let Some(viewport) = &mut camera.viewport {
                viewport.clamp_to_size(new_computed_target_info.physical_size);
            }
            // 此检查是必要的,因为当 WindowMode 更改为全屏时,由于窗口大小突然变为较小值,视口可能具有无效参数
            // 如果窗口大小较小,视口将匹配该较小值
            camera.computed.target_info = Some(new_computed_target_info);
            if let Some(size) = camera.logical_viewport_size()
                && size.x != 0.0
                && size.y != 0.0
            {
                camera_projection.update(size.x, size.y);
                camera.computed.clip_from_view = match &camera.sub_camera_view {
                    Some(sub_view) => camera_projection.get_clip_from_view_for_sub(sub_view),
                    None => camera_projection.get_clip_from_view(),
                }
            }
            // 更新相机投影和裁剪矩阵
        }

        if camera.computed.old_viewport_size != viewport_size {
            camera.computed.old_viewport_size = viewport_size;
        }
        // 更新旧视口大小

        if camera.computed.old_sub_camera_view != camera.sub_camera_view {
            camera.computed.old_sub_camera_view = camera.sub_camera_view;
        }
        // 更新旧子相机视图
    }
    Ok(())
}

/// 提取的相机组件 - 用于渲染世界中的相机数据
#[derive(Component, Debug)]
pub struct ExtractedCamera {
    pub target: Option<NormalizedRenderTarget>,
    // 渲染目标
    pub physical_viewport_size: Option<UVec2>,
    // 视口的物理大小
    pub physical_target_size: Option<UVec2>,
    // 目标的物理大小
    pub viewport: Option<Viewport>,
    pub schedule: InternedScheduleLabel,
    pub order: isize,
    // 渲染顺序
    pub output_mode: CameraOutputMode,
    // 输出模式
    pub msaa_writeback: MsaaWriteback,
    // MSAA 回写设置
    pub clear_color: ClearColorConfig,
    // 清除颜色配置
    pub sorted_camera_index_for_target: usize,
    // 目标的相机排序索引
    pub exposure: f32,
    // 曝光值
    pub hdr: bool,
    // 是否启用 HDR
}

/// 相机提取系统 - 将相机数据从主世界提取到渲染世界
pub fn extract_cameras(
    mut commands: Commands,
    query: Extract<
        Query<(
            Entity,
            RenderEntity,
            &Camera,
            &RenderTarget,
            &CameraRenderGraph,
            &GlobalTransform,
            &VisibleEntities,
            &Frustum,
            (
                Has<Hdr>,
                Option<&ColorGrading>,
                Option<&Exposure>,
                Option<&TemporalJitter>,
                Option<&MipBias>,
                Option<&RenderLayers>,
                Option<&Projection>,
                Has<NoIndirectDrawing>,
            ),
        )>,
    >,
    primary_window: Extract<Query<Entity, With<PrimaryWindow>>>,
    gpu_preprocessing_support: Res<GpuPreprocessingSupport>,
    mapper: Extract<Query<&RenderEntity>>,
) {
    let primary_window = primary_window.iter().next();
    type ExtractedCameraComponents = (
        ExtractedCamera,
        ExtractedView,
        RenderVisibleEntities,
        TemporalJitter,
        MipBias,
        RenderLayers,
        Projection,
        NoIndirectDrawing,
        ViewUniformOffset,
    );
    for (
        main_entity,
        render_entity,
        camera,
        render_target,
        camera_render_graph,
        transform,
        visible_entities,
        frustum,
        (
            hdr,
            color_grading,
            exposure,
            temporal_jitter,
            mip_bias,
            render_layers,
            projection,
            no_indirect_drawing,
        ),
    ) in query.iter()
    {
        if !camera.is_active {
            commands
                .entity(render_entity)
                .remove::<ExtractedCameraComponents>();
            continue;
        }

        let color_grading = color_grading.unwrap_or(&ColorGrading::default()).clone();

        if let (
            Some(URect {
                min: viewport_origin,
                ..
            }),
            Some(viewport_size),
            Some(target_size),
        ) = (
            camera.physical_viewport_rect(),
            camera.physical_viewport_size(),
            camera.physical_target_size(),
        ) {
            if target_size.x == 0 || target_size.y == 0 {
                commands
                    .entity(render_entity)
                    .remove::<ExtractedCameraComponents>();
                continue;
            }

            let render_visible_entities = RenderVisibleEntities {
                entities: visible_entities
                    .entities
                    .iter()
                    .map(|(type_id, entities)| {
                        let entities = entities
                            .iter()
                            .map(|entity| {
                                let render_entity = mapper
                                    .get(*entity)
                                    .cloned()
                                    .map(|entity| entity.id())
                                    .unwrap_or(Entity::PLACEHOLDER);
                                (render_entity, (*entity).into())
                            })
                            .collect();
                        (*type_id, entities)
                    })
                    .collect(),
            };

            let mut commands = commands.entity(render_entity);
            commands.insert((
                ExtractedCamera {
                    target: render_target.normalize(primary_window),
                    viewport: camera.viewport.clone(),
                    physical_viewport_size: Some(viewport_size),
                    physical_target_size: Some(target_size),
                    schedule: camera_render_graph.0,
                    order: camera.order,
                    output_mode: camera.output_mode,
                    msaa_writeback: camera.msaa_writeback,
                    clear_color: camera.clear_color,
                    // this will be set in sort_cameras
                    sorted_camera_index_for_target: 0,
                    exposure: exposure
                        .map(Exposure::exposure)
                        .unwrap_or_else(|| Exposure::default().exposure()),
                    hdr,
                },
                ExtractedView {
                    retained_view_entity: RetainedViewEntity::new(main_entity.into(), None, 0),
                    clip_from_view: camera.clip_from_view(),
                    world_from_view: *transform,
                    clip_from_world: None,
                    hdr,
                    viewport: UVec4::new(
                        viewport_origin.x,
                        viewport_origin.y,
                        viewport_size.x,
                        viewport_size.y,
                    ),
                    color_grading,
                    invert_culling: camera.invert_culling,
                },
                render_visible_entities,
                *frustum,
            ));

            if let Some(temporal_jitter) = temporal_jitter {
                commands.insert(temporal_jitter.clone());
            } else {
                commands.remove::<TemporalJitter>();
            }

            if let Some(mip_bias) = mip_bias {
                commands.insert(mip_bias.clone());
            } else {
                commands.remove::<MipBias>();
            }

            if let Some(render_layers) = render_layers {
                commands.insert(render_layers.clone());
            } else {
                commands.remove::<RenderLayers>();
            }

            if let Some(projection) = projection {
                commands.insert(projection.clone());
            } else {
                commands.remove::<Projection>();
            }

            if no_indirect_drawing
                || !matches!(
                    gpu_preprocessing_support.max_supported_mode,
                    GpuPreprocessingMode::Culling
                )
            {
                commands.insert(NoIndirectDrawing);
            } else {
                commands.remove::<NoIndirectDrawing>();
            }
        };
    }
}

/// Cameras sorted by their order field. This is updated in the [`sort_cameras`] system.
/// 按顺序字段排序的相机.这在 [`sort_cameras`] 系统中更新
#[derive(Resource, Default)]
pub struct SortedCameras(pub Vec<SortedCamera>);

/// 排序后的相机数据
pub struct SortedCamera {
    pub entity: Entity,
    // 相机实体
    pub order: isize,
    // 渲染顺序
    pub target: Option<NormalizedRenderTarget>,
    // 渲染目标
    pub hdr: bool,
    // 是否启用 HDR
}

/// 相机排序系统 - 按顺序字段对相机进行排序
pub fn sort_cameras(
    mut sorted_cameras: ResMut<SortedCameras>,
    mut cameras: Query<(Entity, &mut ExtractedCamera)>,
) {
    sorted_cameras.0.clear();
    for (entity, camera) in cameras.iter() {
        sorted_cameras.0.push(SortedCamera {
            entity,
            order: camera.order,
            target: camera.target.clone(),
            hdr: camera.hdr,
        });
    }
    // sort by order and ensure within an order, RenderTargets of the same type are packed together
    // 按顺序排序,并确保在同一顺序内,相同类型的 RenderTarget 被打包在一起
    sorted_cameras
        .0
        .sort_by(|c1, c2| (c1.order, &c1.target).cmp(&(c2.order, &c2.target)));
    let mut previous_order_target = None;
    let mut ambiguities = <HashSet<_>>::default();
    let mut target_counts = <HashMap<_, _>>::default();
    for sorted_camera in &mut sorted_cameras.0 {
        let new_order_target = (sorted_camera.order, sorted_camera.target.clone());
        if let Some(previous_order_target) = previous_order_target
            && previous_order_target == new_order_target
        {
            ambiguities.insert(new_order_target.clone());
        }
        if let Some(target) = &sorted_camera.target {
            let count = target_counts
                .entry((target.clone(), sorted_camera.hdr))
                .or_insert(0usize);
            let (_, mut camera) = cameras.get_mut(sorted_camera.entity).unwrap();
            camera.sorted_camera_index_for_target = *count;
            *count += 1;
        }
        previous_order_target = Some(new_order_target);
    }

    if !ambiguities.is_empty() {
        warn_once!(
            "Camera order ambiguities detected for active cameras with the following priorities: {:?}. \
            To fix this, ensure there is exactly one Camera entity spawned with a given order for a given RenderTarget. \
            Ambiguities should be resolved because either (1) multiple active cameras were spawned accidentally, which will \
            result in rendering multiple instances of the scene or (2) for cases where multiple active cameras is intentional, \
            ambiguities could result in unpredictable render results.",
            ambiguities
        );
    }
}

/// A subpixel offset to jitter a perspective camera's frustum by.
///
/// Useful for temporal rendering techniques.
#[derive(Component, Clone, Default, Reflect)]
#[reflect(Default, Component, Clone)]
pub struct TemporalJitter {
    /// Offset is in range [-0.5, 0.5].
    pub offset: Vec2,
}

impl TemporalJitter {
    pub fn jitter_projection(&self, clip_from_view: &mut Mat4, view_size: Vec2) {
        // https://github.com/GPUOpen-LibrariesAndSDKs/FidelityFX-SDK/blob/d7531ae47d8b36a5d4025663e731a47a38be882f/docs/techniques/media/super-resolution-temporal/jitter-space.svg
        let mut jitter = (self.offset * vec2(2.0, -2.0)) / view_size;

        // orthographic
        if clip_from_view.w_axis.w == 1.0 {
            jitter *= vec2(clip_from_view.x_axis.x, clip_from_view.y_axis.y) * 0.5;
        }

        clip_from_view.z_axis.x += jitter.x;
        clip_from_view.z_axis.y += jitter.y;
    }
}

/// Camera component specifying a mip bias to apply when sampling from material textures.
///
/// Often used in conjunction with antialiasing post-process effects to reduce textures blurriness.
#[derive(Component, Reflect, Clone)]
#[reflect(Default, Component)]
pub struct MipBias(pub f32);

impl Default for MipBias {
    fn default() -> Self {
        Self(-1.0)
    }
}
