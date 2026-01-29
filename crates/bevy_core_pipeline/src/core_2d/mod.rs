mod main_opaque_pass_2d_node;
mod main_transparent_pass_2d_node;

use core::ops::Range;

use bevy_asset::UntypedAssetId;
use bevy_camera::{Camera, Camera2d};
use bevy_image::ToExtents;
use bevy_platform::collections::{HashMap, HashSet};
use bevy_render::{
    batching::gpu_preprocessing::GpuPreprocessingMode,
    camera::CameraRenderGraph,
    render_phase::PhaseItemBatchSetKey,
    view::{ExtractedView, RetainedViewEntity},
};
pub use main_opaque_pass_2d_node::*;
pub use main_transparent_pass_2d_node::*;

use crate::schedule::Core2d;
use crate::tonemapping::{tonemapping, DebandDither, Tonemapping};
use crate::upscaling::upscaling;
use crate::Core2dSystems;
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use bevy_math::FloatOrd;
use bevy_render::{
    camera::ExtractedCamera,
    extract_component::ExtractComponentPlugin,
    render_phase::{
        sort_phase_system, BinnedPhaseItem, CachedRenderPipelinePhaseItem, DrawFunctionId,
        DrawFunctions, PhaseItem, PhaseItemExtraIndex, SortedPhaseItem, ViewBinnedRenderPhases,
        ViewSortedRenderPhases,
    },
    render_resource::{
        BindGroupId, CachedRenderPipelineId, TextureDescriptor, TextureDimension, TextureFormat,
        TextureUsages,
    },
    renderer::RenderDevice,
    sync_world::MainEntity,
    texture::TextureCache,
    view::{Msaa, ViewDepthTexture},
    Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
};

// 2D 渲染管线使用的深度缓冲格式
// Depth32Float 提供高精度的深度值，适合需要精确深度测试的场景
pub const CORE_2D_DEPTH_FORMAT: TextureFormat = TextureFormat::Depth32Float;

/// 2D 核心渲染插件
/// 
/// 该插件负责初始化和配置 Bevy 的 2D 渲染管线
/// 它会:
/// 1. 注册必需的组件 (DebandDither, CameraRenderGraph, Tonemapping)
/// 2. 初始化绘制函数资源
/// 3. 初始化渲染阶段资源
/// 4. 添加提取系统
/// 5. 添加排序和准备系统
/// 6. 添加核心 2D 调度和系统
pub struct Core2dPlugin;

impl Plugin for Core2dPlugin {
    fn build(&self, app: &mut App) {
        // 注册 Camera2d 组件所需的其他组件
        // 这确保了当实体拥有 Camera2d 组件时，自动获得这些必需的组件
        app.register_required_components::<Camera2d, DebandDither>()
            // 为 Camera2d 注册 CameraRenderGraph 组件，使用 Core2d 渲染图
            .register_required_components_with::<Camera2d, CameraRenderGraph>(|| {
                CameraRenderGraph::new(Core2d)
            })
            // 为 Camera2d 注册 Tonemapping 组件，默认值为 None
            .register_required_components_with::<Camera2d, Tonemapping>(|| Tonemapping::None)
            // 添加 Camera2d 组件的提取插件
            // 提取插件负责将数据从主世界复制到渲染世界
            .add_plugins(ExtractComponentPlugin::<Camera2d>::default());

        // 获取渲染应用子应用
        // 如果不存在渲染应用（例如在服务器模式下），则直接返回
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        
        // 配置渲染应用
        render_app
            // 初始化不透明物体的绘制函数资源
            // DrawFunctions 存储了如何渲染特定类型物体的函数
            .init_resource::<DrawFunctions<Opaque2d>>()
            // 初始化 Alpha 遮罩物体的绘制函数资源
            .init_resource::<DrawFunctions<AlphaMask2d>>()
            // 初始化透明物体的绘制函数资源
            .init_resource::<DrawFunctions<Transparent2d>>()
            // 初始化透明物体的排序渲染阶段资源
            // ViewSortedRenderPhases 存储了按视图组织的、已排序的渲染阶段
            .init_resource::<ViewSortedRenderPhases<Transparent2d>>()
            // 初始化不透明物体的分箱渲染阶段资源
            // ViewBinnedRenderPhases 存储了按视图组织的、已分箱的渲染阶段
            .init_resource::<ViewBinnedRenderPhases<Opaque2d>>()
            // 初始化 Alpha 遮罩物体的分箱渲染阶段资源
            .init_resource::<ViewBinnedRenderPhases<AlphaMask2d>>()
            // 添加提取系统，用于提取 2D 相机的渲染阶段
            // 提取系统在 ExtractSchedule 中运行，将数据从主世界复制到渲染世界
            .add_systems(ExtractSchedule, extract_core_2d_camera_phases)
            // 添加渲染阶段的排序和准备系统
            .add_systems(
                Render,
                (
                    // 对透明物体进行排序
                    // 透明物体需要按深度从后到前排序，以确保正确的混合顺序
                    sort_phase_system::<Transparent2d>.in_set(RenderSystems::PhaseSort),
                    // 准备 2D 深度纹理
                    // 该系统为每个视图创建或更新深度缓冲纹理
                    prepare_core_2d_depth_textures.in_set(RenderSystems::PrepareResources),
                ),
            )
            // 添加 Core2d 基础调度
            // Core2d 调度定义了 2D 渲染的执行顺序
            .add_schedule(Core2d::base_schedule())
            // 添加核心 2D 渲染系统
            .add_systems(
                Core2d,
                (
                    // 主渲染通道：先渲染不透明物体，再渲染透明物体
                    // .chain() 确保两个系统按顺序执行
                    (main_opaque_pass_2d, main_transparent_pass_2d)
                        .chain()
                        .in_set(Core2dSystems::MainPass),
                    // 色调映射后处理
                    // 用于将 HDR 颜色转换为 LDR 颜色，应用色调映射曲线
                    tonemapping.in_set(Core2dSystems::PostProcess),
                    // 放大处理
                    // 在色调映射之后执行，用于处理分辨率缩放等
                    upscaling.after(Core2dSystems::PostProcess),
                ),
            );
    }
}

/// Opaque 2D [`BinnedPhaseItem`]s.
/// 
/// 2D 不透明物体渲染阶段项
/// 
/// 该结构体表示一个待渲染的 2D 不透明物体或批次
/// 它包含了渲染所需的所有信息：
/// - 如何分组（batch_set_key, bin_key）
/// - 从哪里获取数据（representative_entity）
/// - 渲染范围（batch_range）
/// - 额外索引信息（extra_index）
pub struct Opaque2d {
    /// Determines which objects can be placed into a *batch set*.
    ///
    /// Objects in a single batch set can potentially be multi-drawn together,
    /// if it's enabled and the current platform supports it.
    /// 确定哪些对象可以放入*批处理集*
    ///
    /// 如果启用且当前平台支持，单个批处理集中的对象可以一起进行多次绘制
    pub batch_set_key: BatchSetKey2d,
    /// The key, which determines which can be batched.
    /// 用于确定哪些对象可以批处理的键
    ///
    /// 具有相同 bin_key 的对象会被分到同一批，减少渲染状态切换
    pub bin_key: Opaque2dBinKey,
    /// An entity from which data will be fetched, including the mesh if
    /// applicable.
    /// 将从中获取数据的实体（如果适用，包括网格）
    ///
    /// 这个实体包含了渲染该物体所需的所有组件数据
    pub representative_entity: (Entity, MainEntity),
    /// The ranges of instances.
    /// 实例的范围
    ///
    /// 指定要渲染的实例数量范围（用于 instanced rendering）
    pub batch_range: Range<u32>,
    /// An extra index, which is either a dynamic offset or an index in the
    /// indirect parameters list.
    /// 额外索引，要么是动态偏移量，要么是间接参数列表中的索引
    ///
    /// 用于存储额外的渲染参数索引
    pub extra_index: PhaseItemExtraIndex,
}

/// Data that must be identical in order to batch phase items together.
/// 
/// 为了将阶段项批处理在一起必须相同的数据
/// 
/// bin key 用于将可以一起渲染的物体分组
/// 具有相同 bin key 的物体可以共享相同的渲染状态
/// 减少 GPU 状态切换，提高渲染效率
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Opaque2dBinKey {
    /// The identifier of the render pipeline.
    /// 渲染管线的标识符
    ///
    /// 用于查找缓存的渲染管线对象
    pub pipeline: CachedRenderPipelineId,
    /// The function used to draw.
    /// 用于绘制的函数
    ///
    /// 指定如何渲染该物体的绘制函数
    pub draw_function: DrawFunctionId,
    /// The asset that this phase item is associated with.
    ///
    /// Normally, this is the ID of the mesh, but for non-mesh items it might be
    /// the ID of another type of asset.
    /// 与此阶段项关联的资产
    ///
    /// 通常是网格的 ID，但对于非网格项，可能是其他类型资产的 ID
    pub asset_id: UntypedAssetId,
    /// The ID of a bind group specific to the material.
    /// 特定于材质的绑定组 ID
    ///
    /// 用于绑定材质相关的资源（纹理、uniform 等）
    pub material_bind_group_id: Option<BindGroupId>,
}

impl PhaseItem for Opaque2d {
    // 返回该阶段项对应的实体
    #[inline]
    fn entity(&self) -> Entity {
        self.representative_entity.0
    }

    // 返回主世界中的实体（用于同步）
    fn main_entity(&self) -> MainEntity {
        self.representative_entity.1
    }

    // 返回用于绘制该物体的绘制函数 ID
    #[inline]
    fn draw_function(&self) -> DrawFunctionId {
        self.bin_key.draw_function
    }

    // 返回实例范围的引用
    #[inline]
    fn batch_range(&self) -> &Range<u32> {
        &self.batch_range
    }

    // 返回实例范围的可变引用
    #[inline]
    fn batch_range_mut(&mut self) -> &mut Range<u32> {
        &mut self.batch_range
    }

    // 返回额外索引
    fn extra_index(&self) -> PhaseItemExtraIndex {
        self.extra_index.clone()
    }

    // 返回实例范围和额外索引的可变引用
    fn batch_range_and_extra_index_mut(&mut self) -> (&mut Range<u32>, &mut PhaseItemExtraIndex) {
        (&mut self.batch_range, &mut self.extra_index)
    }
}

impl BinnedPhaseItem for Opaque2d {
    // Since 2D meshes presently can't be multidrawn, the batch set key is
    // irrelevant.
    // 由于 2D 网格目前不能进行多次绘制，批处理集键是无关的
    type BatchSetKey = BatchSetKey2d;

    // 分箱键类型，用于将可以批处理的物体分组
    type BinKey = Opaque2dBinKey;

    // 创建新的 Opaque2d 实例
    fn new(
        batch_set_key: Self::BatchSetKey,
        bin_key: Self::BinKey,
        representative_entity: (Entity, MainEntity),
        batch_range: Range<u32>,
        extra_index: PhaseItemExtraIndex,
    ) -> Self {
        Opaque2d {
            batch_set_key,
            bin_key,
            representative_entity,
            batch_range,
            extra_index,
        }
    }
}

/// 2D meshes aren't currently multi-drawn together, so this batch set key only
/// stores whether the mesh is indexed.
/// 
/// 2D 网格目前不一起进行多次绘制，因此此批处理集键仅存储网格是否被索引
/// 
/// 批处理集键用于确定哪些对象可以放入同一批处理集
/// 在 2D 渲染中，它主要用于标识网格是否使用索引绘制
#[derive(Clone, Copy, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct BatchSetKey2d {
    /// True if the mesh is indexed.
    /// 如果网格被索引则为 true
    pub indexed: bool,
}

impl PhaseItemBatchSetKey for BatchSetKey2d {
    // 返回网格是否被索引
    fn indexed(&self) -> bool {
        self.indexed
    }
}

impl CachedRenderPipelinePhaseItem for Opaque2d {
    // 返回缓存的渲染管线 ID
    #[inline]
    fn cached_pipeline(&self) -> CachedRenderPipelineId {
        self.bin_key.pipeline
    }
}

/// Alpha mask 2D [`BinnedPhaseItem`]s.
/// 
/// 2D Alpha 遮罩物体渲染阶段项
/// 
/// Alpha 遮罩物体是指那些使用 Alpha 通道进行遮罩测试的物体
/// 它们会根据 Alpha 值决定是否绘制像素（通常用于实现镂空效果）
/// 与透明物体不同，Alpha 遮罩物体不需要按深度排序
pub struct AlphaMask2d {
    /// Determines which objects can be placed into a *batch set*.
    ///
    /// Objects in a single batch set can potentially be multi-drawn together,
    /// if it's enabled and the current platform supports it.
    /// 确定哪些对象可以放入*批处理集*
    ///
    /// 如果启用且当前平台支持，单个批处理集中的对象可以一起进行多次绘制
    pub batch_set_key: BatchSetKey2d,
    /// The key, which determines which can be batched.
    /// 用于确定哪些对象可以批处理的键
    ///
    /// 具有相同 bin_key 的对象会被分到同一批，减少渲染状态切换
    pub bin_key: AlphaMask2dBinKey,
    /// An entity from which data will be fetched, including the mesh if
    /// applicable.
    /// 将从中获取数据的实体（如果适用，包括网格）
    ///
    /// 这个实体包含了渲染该物体所需的所有组件数据
    pub representative_entity: (Entity, MainEntity),
    /// The ranges of instances.
    /// 实例的范围
    ///
    /// 指定要渲染的实例数量范围（用于 instanced rendering）
    pub batch_range: Range<u32>,
    /// An extra index, which is either a dynamic offset or an index in the
    /// indirect parameters list.
    /// 额外索引，要么是动态偏移量，要么是间接参数列表中的索引
    ///
    /// 用于存储额外的渲染参数索引
    pub extra_index: PhaseItemExtraIndex,
}

/// Data that must be identical in order to batch phase items together.
/// 
/// 为了将 Alpha 遮罩阶段项批处理在一起必须相同的数据
/// 
/// 结构与 Opaque2dBinKey 相同，但用于 Alpha 遮罩物体
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AlphaMask2dBinKey {
    /// The identifier of the render pipeline.
    /// 渲染管线的标识符
    pub pipeline: CachedRenderPipelineId,
    /// The function used to draw.
    /// 用于绘制的函数
    pub draw_function: DrawFunctionId,
    /// The asset that this phase item is associated with.
    ///
    /// Normally, this is the ID of the mesh, but for non-mesh items it might be
    /// the ID of another type of asset.
    /// 与此阶段项关联的资产
    pub asset_id: UntypedAssetId,
    /// The ID of a bind group specific to the material.
    /// 特定于材质的绑定组 ID
    pub material_bind_group_id: Option<BindGroupId>,
}

impl PhaseItem for AlphaMask2d {
    #[inline]
    fn entity(&self) -> Entity {
        self.representative_entity.0
    }

    #[inline]
    fn main_entity(&self) -> MainEntity {
        self.representative_entity.1
    }

    #[inline]
    fn draw_function(&self) -> DrawFunctionId {
        self.bin_key.draw_function
    }

    #[inline]
    fn batch_range(&self) -> &Range<u32> {
        &self.batch_range
    }

    #[inline]
    fn batch_range_mut(&mut self) -> &mut Range<u32> {
        &mut self.batch_range
    }

    fn extra_index(&self) -> PhaseItemExtraIndex {
        self.extra_index.clone()
    }

    fn batch_range_and_extra_index_mut(&mut self) -> (&mut Range<u32>, &mut PhaseItemExtraIndex) {
        (&mut self.batch_range, &mut self.extra_index)
    }
}

impl BinnedPhaseItem for AlphaMask2d {
    // Since 2D meshes presently can't be multidrawn, the batch set key is
    // irrelevant.
    type BatchSetKey = BatchSetKey2d;

    type BinKey = AlphaMask2dBinKey;

    fn new(
        batch_set_key: Self::BatchSetKey,
        bin_key: Self::BinKey,
        representative_entity: (Entity, MainEntity),
        batch_range: Range<u32>,
        extra_index: PhaseItemExtraIndex,
    ) -> Self {
        AlphaMask2d {
            batch_set_key,
            bin_key,
            representative_entity,
            batch_range,
            extra_index,
        }
    }
}

impl CachedRenderPipelinePhaseItem for AlphaMask2d {
    #[inline]
    fn cached_pipeline(&self) -> CachedRenderPipelineId {
        self.bin_key.pipeline
    }
}

/// Transparent 2D [`SortedPhaseItem`]s.
/// 
/// 2D 透明物体渲染阶段项
/// 
/// 透明物体需要按深度排序，以确保正确的混合顺序
/// 它们使用 Alpha 混合将颜色与帧缓冲中的现有颜色混合
pub struct Transparent2d {
    /// 用于排序的键
    /// 决定透明物体的渲染顺序（从后到前）
    pub sort_key: FloatOrd,
    /// 将从中获取数据的实体
    pub entity: (Entity, MainEntity),
    /// 渲染管线的标识符
    pub pipeline: CachedRenderPipelineId,
    /// 用于绘制的函数
    pub draw_function: DrawFunctionId,
    /// 实例的范围
    pub batch_range: Range<u32>,
    /// 提取的索引
    pub extracted_index: usize,
    /// 额外索引，要么是动态偏移量，要么是间接参数列表中的索引
    pub extra_index: PhaseItemExtraIndex,
    /// 网格是否被索引（除顶点缓冲区外还使用索引缓冲区）
    pub indexed: bool,
}

impl PhaseItem for Transparent2d {
    #[inline]
    fn entity(&self) -> Entity {
        self.entity.0
    }

    #[inline]
    fn main_entity(&self) -> MainEntity {
        self.entity.1
    }

    #[inline]
    fn draw_function(&self) -> DrawFunctionId {
        self.draw_function
    }

    #[inline]
    fn batch_range(&self) -> &Range<u32> {
        &self.batch_range
    }

    #[inline]
    fn batch_range_mut(&mut self) -> &mut Range<u32> {
        &mut self.batch_range
    }

    #[inline]
    fn extra_index(&self) -> PhaseItemExtraIndex {
        self.extra_index.clone()
    }

    #[inline]
    fn batch_range_and_extra_index_mut(&mut self) -> (&mut Range<u32>, &mut PhaseItemExtraIndex) {
        (&mut self.batch_range, &mut self.extra_index)
    }
}

impl SortedPhaseItem for Transparent2d {
    type SortKey = FloatOrd;

    #[inline]
    fn sort_key(&self) -> Self::SortKey {
        self.sort_key
    }

    #[inline]
    fn sort(items: &mut [Self]) {
        // radsort is a stable radix sort that performed better than `slice::sort_by_key` or `slice::sort_unstable_by_key`.
        radsort::sort_by_key(items, |item| item.sort_key().0);
    }

    fn indexed(&self) -> bool {
        self.indexed
    }
}

impl CachedRenderPipelinePhaseItem for Transparent2d {
    #[inline]
    fn cached_pipeline(&self) -> CachedRenderPipelineId {
        self.pipeline
    }
}

/// 提取 2D 相机的渲染阶段
/// 
/// 该系统在提取阶段运行，负责初始化和清理每个 2D 相机的渲染阶段数据
/// 
/// 主要职责：
/// 1. 清除上一帧的活跃实体记录
/// 2. 遍历所有活跃的 2D 相机
/// 3. 为每个相机创建或清除对应的渲染阶段
/// 4. 清理不再活跃的相机的渲染阶段数据
pub fn extract_core_2d_camera_phases(
    mut transparent_2d_phases: ResMut<ViewSortedRenderPhases<Transparent2d>>,
    mut opaque_2d_phases: ResMut<ViewBinnedRenderPhases<Opaque2d>>,
    mut alpha_mask_2d_phases: ResMut<ViewBinnedRenderPhases<AlphaMask2d>>,
    cameras_2d: Extract<Query<(Entity, &Camera), With<Camera2d>>>,
    mut live_entities: Local<HashSet<RetainedViewEntity>>,
) {
    live_entities.clear();

    for (main_entity, camera) in &cameras_2d {
        if !camera.is_active {
            continue;
        }

        // 这是主 2D 相机，因此我们使用第一个子视图索引 (0)
        let retained_view_entity = RetainedViewEntity::new(main_entity.into(), None, 0);

        // 插入或清除透明物体的渲染阶段
        transparent_2d_phases.insert_or_clear(retained_view_entity);
        // 为不透明物体的渲染阶段准备新帧
        opaque_2d_phases.prepare_for_new_frame(retained_view_entity, GpuPreprocessingMode::None);
        // 为 Alpha 遮罩物体的渲染阶段准备新帧
        alpha_mask_2d_phases
            .prepare_for_new_frame(retained_view_entity, GpuPreprocessingMode::None);

        live_entities.insert(retained_view_entity);
    }

    // 清除所有不再活跃的视图的渲染阶段数据
    transparent_2d_phases.retain(|camera_entity, _| live_entities.contains(camera_entity));
    opaque_2d_phases.retain(|camera_entity, _| live_entities.contains(camera_entity));
    alpha_mask_2d_phases.retain(|camera_entity, _| live_entities.contains(camera_entity));
}

/// Prepares depth textures for 2D rendering.
/// 
/// This system runs during the prepare resources phase and is responsible for creating or updating
/// depth buffer textures for each 2D camera.
/// 
/// The depth buffer is used for:
/// 1. Depth testing: determining which pixels are visible
/// 2. Depth sorting: for correct rendering order of transparent objects
///
/// 准备 2D 渲染的深度纹理。
/// 
/// 该系统在准备资源阶段运行，负责为每个 2D 相机创建或更新深度缓冲纹理。
/// 
/// 深度缓冲用于：
/// 1. 深度测试：确定哪些像素可见
/// 2. 深度排序：用于透明物体的正确渲染顺序
pub fn prepare_core_2d_depth_textures(
    mut commands: Commands,
    mut texture_cache: ResMut<TextureCache>,
    render_device: Res<RenderDevice>,
    transparent_2d_phases: Res<ViewSortedRenderPhases<Transparent2d>>,
    opaque_2d_phases: Res<ViewBinnedRenderPhases<Opaque2d>>,
    views_2d: Query<(Entity, &ExtractedCamera, &ExtractedView, &Msaa), (With<Camera2d>,)>,
) {
    // 缓存纹理，避免为每个相机重复创建相同的深度纹理
    let mut textures = <HashMap<_, _>>::default();
    
    // 遍历所有 2D 相机
    for (view, camera, extracted_view, msaa) in &views_2d {
        // 跳过没有渲染阶段的相机
        if !opaque_2d_phases.contains_key(&extracted_view.retained_view_entity)
            || !transparent_2d_phases.contains_key(&extracted_view.retained_view_entity)
        {
            continue;
        };

        // 跳过没有目标尺寸的相机
        let Some(physical_target_size) = camera.physical_target_size else {
            continue;
        };

        // 获取或创建深度纹理：
        // - 按 target 分组，相同 target 的相机共享深度纹理
        // - 使用 texture_cache 避免重复创建相同描述符的纹理
        let cached_texture = textures
            .entry(camera.target.clone())
            .or_insert_with(|| {
                let descriptor = TextureDescriptor {
                    label: Some("view_depth_texture"),
                    // The size of the depth texture
                    // 深度纹理的尺寸（与渲染目标相同）
                    size: physical_target_size.to_extents(),
                    mip_level_count: 1,
                    sample_count: msaa.samples(),
                    dimension: TextureDimension::D2,
                    format: CORE_2D_DEPTH_FORMAT,
                    usage: TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                };

                texture_cache.get(&render_device, descriptor)
            })
            .clone();

        // 将深度纹理组件插入到相机实体中
        commands
            .entity(view)
            .insert(ViewDepthTexture::new(cached_texture, Some(0.0)));
    }
}
