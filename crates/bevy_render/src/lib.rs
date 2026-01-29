//! # Useful Environment Variables
//! # 有用的环境变量
//!
//! Both `bevy_render` and `wgpu` have a number of environment variable options for changing the runtime behavior
//! of both crates. Many of these may be useful in development or release environments.
//! `bevy_render` 和 `wgpu` 都有许多环境变量选项,用于更改两个 crate 的运行时行为.其中许多在开发或发布环境中可能很有用
//!
//! - `WGPU_DEBUG=1` enables debug labels, which can be useful in release builds.
//! - `WGPU_DEBUG=1` 启用调试标签,这在发布版本中可能很有用
//! - `WGPU_VALIDATION=0` disables validation layers. This can help with particularly spammy errors.
//! - `WGPU_VALIDATION=0` 禁用验证层.这可以帮助处理特别频繁的错误
//! - `WGPU_FORCE_FALLBACK_ADAPTER=1` attempts to force software rendering. This typically matches what is used in CI.
//! - `WGPU_FORCE_FALLBACK_ADAPTER=1` 尝试强制使用软件渲染.这通常与 CI 中使用的渲染方式匹配
//! - `WGPU_ADAPTER_NAME` allows selecting a specific adapter by name.
//! - `WGPU_ADAPTER_NAME` 允许按名称选择特定的适配器
//! - `WGPU_SETTINGS_PRIO=webgl2` uses webgl2 limits.
//! - `WGPU_SETTINGS_PRIO=webgl2` 使用 webgl2 限制
//! - `WGPU_SETTINGS_PRIO=compatibility` uses webgpu limits.
//! - `WGPU_SETTINGS_PRIO=compatibility` 使用 webgpu 限制
//! - `VERBOSE_SHADER_ERROR=1` prints more detailed information about WGSL compilation errors, such as shader defs and shader entrypoint.
//! - `VERBOSE_SHADER_ERROR=1` 打印有关 WGSL 编译错误的更详细信息,例如 shader 定义和 shader 入口点

#![expect(missing_docs, reason = "Not all docs are written yet, see #3492.")]
#![expect(unsafe_code, reason = "Unsafe code is used to improve performance.")]
#![cfg_attr(
    any(docsrs, docsrs_dep),
    expect(
        internal_features,
        reason = "rustdoc_internals is needed for fake_variadic"
    )
)]
#![cfg_attr(any(docsrs, docsrs_dep), feature(doc_cfg, rustdoc_internals))]
#![doc(
    html_logo_url = "https://bevy.org/assets/icon.png",
    html_favicon_url = "https://bevy.org/assets/icon.png"
)]

#[cfg(target_pointer_width = "16")]
compile_error!("bevy_render cannot compile for a 16-bit platform.");

extern crate alloc;
extern crate core;

// Required to make proc macros work in bevy itself.
extern crate self as bevy_render;

pub mod batching;
// 批处理模块 - 用于优化渲染性能的批处理系统
pub mod camera;
// 相机模块 - 相机系统和视图管理
pub mod diagnostic;
// 诊断模块 - 渲染性能诊断和统计
pub mod erased_render_asset;
// 擦除的渲染资源模块 - 类型擦除的渲染资源管理
pub mod experimental;
// 实验性模块 - 包含实验性的渲染功能
pub mod extract_component;
// 提取组件模块 - 从主世界提取组件到渲染世界
pub mod extract_instances;
// 提取实例模块 - 提取实例数据
mod extract_param;
// 提取参数模块 - 系统参数的提取逻辑
pub mod extract_resource;
// 提取资源模块 - 从主世界提取资源到渲染世界
pub mod globals;
// 全局变量模块 - 全局着色器变量和 uniforms
pub mod gpu_component_array_buffer;
// GPU 组件数组缓冲区模块 - 在 GPU 上存储组件数组
pub mod gpu_readback;
// GPU 回读模块 - 从 GPU 读取数据回 CPU
pub mod mesh;
// 网格模块 - 网格数据结构和渲染
#[cfg(not(target_arch = "wasm32"))]
pub mod pipelined_rendering;
// 流水线渲染模块 - 仅在非 WASM 平台可用的流水线渲染
pub mod render_asset;
// 渲染资源模块 - 渲染资源的准备和管理
pub mod render_phase;
// 渲染阶段模块 - 渲染阶段和绘制逻辑
pub mod render_resource;
// 渲染资源模块 - GPU 资源的抽象和管理
pub mod renderer;
// 渲染器模块 - 核心渲染器实现
pub mod settings;
// 设置模块 - 渲染设置和配置
pub mod storage;
// 存储模块 - 渲染数据的存储管理
pub mod sync_component;
// 同步组件模块 - 组件同步逻辑
pub mod sync_world;
// 同步世界模块 - 世界同步逻辑
pub mod texture;
// 纹理模块 - 纹理数据结构和管理
pub mod view;
// 视图模块 - 视图和视口管理

/// The render prelude.
/// 渲染预导入模块
///
/// This includes the most common types in this crate, re-exported for your convenience.
/// 包含此 crate 中最常用的类型,为方便起见重新导出
pub mod prelude {
    #[doc(hidden)]
    pub use crate::{
        camera::NormalizedRenderTargetExt as _, texture::ManualTextureViews, view::Msaa,
        ExtractSchedule,
    };
}

pub use extract_param::Extract;
// 导出 Extract 类型 - 用于提取系统参数

use crate::{
    camera::CameraPlugin,
    // 相机插件 - 相机系统的核心插件
    gpu_readback::GpuReadbackPlugin,
    // GPU 回读插件 - 用于从 GPU 读取数据
    mesh::{MeshRenderAssetPlugin, RenderMesh},
    // 网格渲染资源插件和渲染网格类型
    render_asset::prepare_assets,
    // 准备资源函数 - 准备渲染资源
    render_resource::PipelineCache,
    // 管线缓存 - 管理渲染管线的缓存
    renderer::{render_system, RenderAdapterInfo},
    // 渲染系统和适配器信息
    settings::RenderCreation,
    // 渲染创建设置 - 渲染器的创建配置
    storage::StoragePlugin,
    // 存储插件 - 存储管理插件
    texture::TexturePlugin,
    // 纹理插件 - 纹理系统插件
    view::{ViewPlugin, WindowRenderPlugin},
    // 视图插件和窗口渲染插件
};
use alloc::sync::Arc;
use batching::gpu_preprocessing::BatchingPlugin;
// 批处理插件 - GPU 预处理批处理
use bevy_app::{App, AppLabel, Plugin, SubApp};
use bevy_asset::{AssetApp, AssetServer};
use bevy_ecs::{
    prelude::*,
    schedule::{ScheduleBuildSettings, ScheduleLabel},
};
use bevy_image::{CompressedImageFormatSupport, CompressedImageFormats};
use bevy_shader::{load_shader_library, Shader, ShaderLoader};
use bevy_utils::prelude::default;
use bevy_window::{PrimaryWindow, RawHandleWrapperHolder};
use bitflags::bitflags;
use core::ops::{Deref, DerefMut};
use experimental::occlusion_culling::OcclusionCullingPlugin;
// 遮挡剔除插件 - 实验性的遮挡剔除功能
use globals::GlobalsPlugin;
// 全局变量插件 - 全局着色器变量管理
use render_asset::{
    extract_render_asset_bytes_per_frame, reset_render_asset_bytes_per_frame,
    RenderAssetBytesPerFrame, RenderAssetBytesPerFrameLimiter,
};
use settings::RenderResources;
use std::sync::Mutex;
use sync_world::{despawn_temporary_render_entities, entity_sync_system, SyncWorldPlugin};
// 同步世界插件 - 主世界和渲染世界的同步

/// Contains the default Bevy rendering backend based on wgpu.
/// 包含基于 wgpu 的默认 Bevy 渲染后端
///
/// Rendering is done in a [`SubApp`], which exchanges data with the main app
/// between main schedule iterations.
/// 渲染在一个 [`SubApp`] 中完成,它在主调度迭代之间与主应用交换数据
///
/// Rendering can be executed between iterations of the main schedule,
/// or it can be executed in parallel with main schedule when
/// [`PipelinedRenderingPlugin`](pipelined_rendering::PipelinedRenderingPlugin) is enabled.
/// 渲染可以在主调度的迭代之间执行,或者当启用 [`PipelinedRenderingPlugin`] 时可以与主调度并行执行
#[derive(Default)]
pub struct RenderPlugin {
    pub render_creation: RenderCreation,
    /// If `true`, disables asynchronous pipeline compilation.
    /// 如果为 `true`,禁用异步管线编译
    /// This has no effect on macOS, Wasm, iOS, or without the `multi_threaded` feature.
    /// 这在 macOS、Wasm、iOS 或没有 `multi_threaded` 特性时无效
    pub synchronous_pipeline_compilation: bool,
    /// Debugging flags that can optionally be set when constructing the renderer.
    /// 构造渲染器时可以选择设置的调试标志
    pub debug_flags: RenderDebugFlags,
}

bitflags! {
    /// Debugging flags that can optionally be set when constructing the renderer.
    /// 构造渲染器时可以选择设置的调试标志
    #[derive(Clone, Copy, PartialEq, Default, Debug)]
    pub struct RenderDebugFlags: u8 {
        /// If true, this sets the `COPY_SRC` flag on indirect draw parameters
        /// so that they can be read back to CPU.
        /// 如果为 true,这会在间接绘制参数上设置 `COPY_SRC` 标志,以便它们可以被回读到 CPU
        ///
        /// This is a debugging feature that may reduce performance. It
        /// primarily exists for the `occlusion_culling` example.
        /// 这是一个可能会降低性能的调试功能.它主要是为 `occlusion_culling` 示例而存在的
        const ALLOW_COPIES_FROM_INDIRECT_PARAMETERS = 1;
    }
}

/// The systems sets of the default [`App`] rendering schedule.
/// 默认 [`App`] 渲染调度的系统集合
///
/// These can be useful for ordering, but you almost never want to add your systems to these sets.
/// 这些对于排序很有用,但你几乎永远不会想把你的系统添加到这些集合中
#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub enum RenderSystems {
    /// This is used for applying the commands from the [`ExtractSchedule`]
    /// 用于应用来自 [`ExtractSchedule`] 的命令
    ExtractCommands,
    /// Prepare assets that have been created/modified/removed this frame.
    /// 准备本帧中已创建/修改/删除的资源
    PrepareAssets,
    /// Prepares extracted meshes.
    /// 准备提取的网格
    PrepareMeshes,
    /// Create any additional views such as those used for shadow mapping.
    /// 创建任何额外的视图,例如用于阴影映射的视图
    ManageViews,
    /// Queue drawable entities as phase items in render phases ready for
    /// sorting (if necessary)
    /// 将可绘制实体作为阶段项排队到渲染阶段中,准备进行排序(如有必要)
    Queue,
    /// A sub-set within [`Queue`](RenderSystems::Queue) where mesh entity queue systems are executed. Ensures `prepare_assets::<RenderMesh>` is completed.
    /// [`Queue`] 中的一个子集,其中执行网格实体排队系统.确保 `prepare_assets::<RenderMesh>` 已完成
    QueueMeshes,
    /// A sub-set within [`Queue`](RenderSystems::Queue) where meshes that have
    /// become invisible or changed phases are removed from the bins.
    /// [`Queue`] 中的一个子集,其中已变为不可见或已更改阶段的网格将从分箱中移除
    QueueSweep,
    // TODO: This could probably be moved in favor of a system ordering
    // abstraction in `Render` or `Queue`
    /// Sort the [`SortedRenderPhase`](render_phase::SortedRenderPhase)s and
    /// [`BinKey`](render_phase::BinnedPhaseItem::BinKey)s here.
    /// 在此处对 [`SortedRenderPhase`] 和 [`BinKey`] 进行排序
    PhaseSort,
    /// Prepare render resources from extracted data for the GPU based on their sorted order.
    /// Create [`BindGroups`](render_resource::BindGroup) that depend on those data.
    /// 根据排序后的顺序从提取的数据中为 GPU 准备渲染资源.创建依赖于这些数据的 [`BindGroups`]
    Prepare,
    /// A sub-set within [`Prepare`](RenderSystems::Prepare) for initializing buffers, textures and uniforms for use in bind groups.
    /// [`Prepare`] 中的一个子集,用于初始化绑定组中使用的缓冲区、纹理和 uniforms
    PrepareResources,
    /// Collect phase buffers after
    /// [`PrepareResources`](RenderSystems::PrepareResources) has run.
    /// 在 [`PrepareResources`] 运行后收集阶段缓冲区
    PrepareResourcesCollectPhaseBuffers,
    /// Flush buffers after [`PrepareResources`](RenderSystems::PrepareResources), but before [`PrepareBindGroups`](RenderSystems::PrepareBindGroups).
    /// 在 [`PrepareResources`] 之后但在 [`PrepareBindGroups`] 之前刷新缓冲区
    PrepareResourcesFlush,
    /// A sub-set within [`Prepare`](RenderSystems::Prepare) for constructing bind groups, or other data that relies on render resources prepared in [`PrepareResources`](RenderSystems::PrepareResources).
    /// [`Prepare`] 中的一个子集,用于构造绑定组或其他依赖于 [`PrepareResources`] 中准备的渲染资源的数据
    PrepareBindGroups,
    /// Actual rendering happens here.
    /// 实际的渲染在此处发生
    /// In most cases, only the render backend should insert resources here.
    /// 在大多数情况下,只有渲染后端应该在此处插入资源
    Render,
    /// Cleanup render resources here.
    /// 在此处清理渲染资源
    Cleanup,
    /// Final cleanup occurs: any entities with
    /// [`TemporaryRenderEntity`](sync_world::TemporaryRenderEntity) will be despawned.
    /// 最终清理发生:任何带有 [`TemporaryRenderEntity`] 的实体都将被移除
    ///
    /// Runs after [`Cleanup`](RenderSystems::Cleanup).
    /// 在 [`Cleanup`] 之后运行
    PostCleanup,
}

/// The startup schedule of the [`RenderApp`]
/// [`RenderApp`] 的启动调度
#[derive(ScheduleLabel, Debug, Hash, PartialEq, Eq, Clone, Default)]
pub struct RenderStartup;

/// The main render schedule.
/// 主渲染调度
#[derive(ScheduleLabel, Debug, Hash, PartialEq, Eq, Clone, Default)]
pub struct Render;

impl Render {
    /// Sets up the base structure of the rendering [`Schedule`].
    /// 设置渲染 [`Schedule`] 的基本结构
    ///
    /// The sets defined in this enum are configured to run in order.
    /// 此枚举中定义的集合被配置为按顺序运行
    pub fn base_schedule() -> Schedule {
        use RenderSystems::*;

        let mut schedule = Schedule::new(Self);

        schedule.configure_sets(
            (
                ExtractCommands,
                PrepareMeshes,
                ManageViews,
                Queue,
                PhaseSort,
                Prepare,
                Render,
                Cleanup,
                PostCleanup,
            )
                .chain(),
        );

        schedule.configure_sets((ExtractCommands, PrepareAssets, PrepareMeshes, Prepare).chain());
        schedule.configure_sets(
            (QueueMeshes, QueueSweep)
                .chain()
                .in_set(Queue)
                .after(prepare_assets::<RenderMesh>),
        );
        schedule.configure_sets(
            (
                PrepareResources,
                PrepareResourcesCollectPhaseBuffers,
                PrepareResourcesFlush,
                PrepareBindGroups,
            )
                .chain()
                .in_set(Prepare),
        );

        schedule
    }
}

/// Schedule in which data from the main world is 'extracted' into the render world.
/// 用于将主世界的数据"提取"到渲染世界的调度
///
/// This step should be kept as short as possible to increase the "pipelining potential" for
/// running the next frame while rendering the current frame.
/// 此步骤应尽可能短,以增加在渲染当前帧时运行下一帧的"流水线潜力"
///
/// This schedule is run on the render world, but it also has access to the main world.
/// See [`MainWorld`] and [`Extract`] for details on how to access main world data from this schedule.
/// 此调度在渲染世界上运行,但它也可以访问主世界.有关如何从此调度访问主世界数据的详细信息,请参见 [`MainWorld`] 和 [`Extract`]
#[derive(ScheduleLabel, PartialEq, Eq, Debug, Clone, Hash, Default)]
pub struct ExtractSchedule;

/// The simulation [`World`] of the application, stored as a resource.
/// 应用程序的模拟 [`World`],作为资源存储
///
/// This resource is only available during [`ExtractSchedule`] and not
/// during command application of that schedule.
/// See [`Extract`] for more details.
/// 此资源仅在 [`ExtractSchedule`] 期间可用,而不在该调度的命令应用期间可用.有关更多详细信息,请参见 [`Extract`]
#[derive(Resource, Default)]
pub struct MainWorld(World);

impl Deref for MainWorld {
    type Target = World;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for MainWorld {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Resource)]
struct FutureRenderResources(Arc<Mutex<Option<RenderResources>>>);
// 未来的渲染资源 - 用于在初始化阶段传递渲染资源

/// A label for the rendering sub-app.
/// 渲染子应用的标签
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, AppLabel)]
pub struct RenderApp;

impl Plugin for RenderPlugin {
    /// Initializes the renderer, sets up the [`RenderSystems`] and creates the rendering sub-app.
    /// 初始化渲染器,设置 [`RenderSystems`] 并创建渲染子应用
    fn build(&self, app: &mut App) {
        app.init_asset::<Shader>()
            .init_asset_loader::<ShaderLoader>();

        match &self.render_creation {
            RenderCreation::Manual(resources) => {
                let future_render_resources_wrapper = Arc::new(Mutex::new(Some(resources.clone())));
                app.insert_resource(FutureRenderResources(
                    future_render_resources_wrapper.clone(),
                ));
                // SAFETY: Plugins should be set up on the main thread.
                // 安全:插件应该在主线程上设置
                unsafe { initialize_render_app(app) };
            }
            RenderCreation::Automatic(render_creation) => {
                if let Some(backends) = render_creation.backends {
                    let future_render_resources_wrapper = Arc::new(Mutex::new(None));
                    app.insert_resource(FutureRenderResources(
                        future_render_resources_wrapper.clone(),
                    ));

                    let primary_window = app
                        .world_mut()
                        .query_filtered::<&RawHandleWrapperHolder, With<PrimaryWindow>>()
                        .single(app.world())
                        .ok()
                        .cloned();
                    // 获取主窗口的原始句柄

                    let settings = render_creation.clone();

                    #[cfg(feature = "raw_vulkan_init")]
                    let raw_vulkan_init_settings = app
                        .world_mut()
                        .get_resource::<renderer::raw_vulkan_init::RawVulkanInitSettings>()
                        .cloned()
                        .unwrap_or_default();
                    // 获取原始 Vulkan 初始化设置

                    let async_renderer = async move {
                        let render_resources = renderer::initialize_renderer(
                            backends,
                            primary_window,
                            &settings,
                            #[cfg(feature = "raw_vulkan_init")]
                            raw_vulkan_init_settings,
                        )
                        .await;
                    // 异步初始化渲染器

                        *future_render_resources_wrapper.lock().unwrap() = Some(render_resources);
                    };

                    // In wasm, spawn a task and detach it for execution
                    // 在 WASM 上,生成一个任务并分离它以执行
                    #[cfg(target_arch = "wasm32")]
                    bevy_tasks::IoTaskPool::get()
                        .spawn_local(async_renderer)
                        .detach();
                    // Otherwise, just block for it to complete
                    // 否则,只需阻塞直到完成
                    #[cfg(not(target_arch = "wasm32"))]
                    bevy_tasks::block_on(async_renderer);

                    // SAFETY: Plugins should be set up on the main thread.
                    // 安全:插件应该在主线程上设置
                    unsafe { initialize_render_app(app) };
                }
            }
        };

        app.add_plugins((
            WindowRenderPlugin,
            // 窗口渲染插件
            CameraPlugin,
            // 相机插件
            ViewPlugin,
            // 视图插件
            MeshRenderAssetPlugin,
            // 网格渲染资源插件
            GlobalsPlugin,
            // 全局变量插件
            #[cfg(feature = "morph")]
            mesh::MorphPlugin,
            // 变形插件
            TexturePlugin,
            // 纹理插件
            BatchingPlugin {
                debug_flags: self.debug_flags,
            },
            // 批处理插件
            SyncWorldPlugin,
            // 同步世界插件
            StoragePlugin,
            // 存储插件
            GpuReadbackPlugin::default(),
            // GPU 回读插件
            OcclusionCullingPlugin,
            // 遮挡剔除插件
            #[cfg(feature = "tracing-tracy")]
            diagnostic::RenderDiagnosticsPlugin,
            // 渲染诊断插件
        ));

        app.init_resource::<RenderAssetBytesPerFrame>();
        // 初始化每帧渲染资源字节数统计
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.init_resource::<RenderAssetBytesPerFrameLimiter>();
            render_app
                .add_systems(ExtractSchedule, extract_render_asset_bytes_per_frame)
                .add_systems(
                    Render,
                    reset_render_asset_bytes_per_frame.in_set(RenderSystems::Cleanup),
                );
        }
    }

    fn ready(&self, app: &App) -> bool {
        // 检查渲染器是否已准备好
        app.world()
            .get_resource::<FutureRenderResources>()
            .and_then(|frr| frr.0.try_lock().map(|locked| locked.is_some()).ok())
            .unwrap_or(true)
    }

    fn finish(&self, app: &mut App) {
        // 加载内置着色器库
        load_shader_library!(app, "maths.wgsl");
        load_shader_library!(app, "color_operations.wgsl");
        load_shader_library!(app, "bindless.wgsl");
        if let Some(future_render_resources) =
            app.world_mut().remove_resource::<FutureRenderResources>()
        {
            let render_resources = future_render_resources.0.lock().unwrap().take().unwrap();
            let RenderResources(device, queue, adapter_info, render_adapter, instance, ..) =
                render_resources;

            let compressed_image_format_support = CompressedImageFormatSupport(
                CompressedImageFormats::from_features(device.features()),
            );
            // 检查压缩图像格式支持

            app.insert_resource(device.clone())
                .insert_resource(queue.clone())
                .insert_resource(adapter_info.clone())
                .insert_resource(render_adapter.clone())
                .insert_resource(compressed_image_format_support);
            // 将渲染资源插入主应用

            let render_app = app.sub_app_mut(RenderApp);

            #[cfg(feature = "raw_vulkan_init")]
            {
                let additional_vulkan_features: renderer::raw_vulkan_init::AdditionalVulkanFeatures =
                    render_resources.5;
                render_app.insert_resource(additional_vulkan_features);
            }
            // 插入额外的 Vulkan 特性

            render_app
                .insert_resource(instance)
                .insert_resource(PipelineCache::new(
                    device.clone(),
                    render_adapter.clone(),
                    self.synchronous_pipeline_compilation,
                ))
                .insert_resource(device)
                .insert_resource(queue)
                .insert_resource(render_adapter)
                .insert_resource(adapter_info);
            // 将渲染资源插入渲染子应用
        }
    }
}

/// A "scratch" world used to avoid allocating new worlds every frame when
/// swapping out the [`MainWorld`] for [`ExtractSchedule`].
/// 一个"临时"世界,用于避免在为 [`ExtractSchedule`] 交换 [`MainWorld`] 时每帧分配新世界
#[derive(Resource, Default)]
struct ScratchMainWorld(World);

/// Executes the [`ExtractSchedule`] step of the renderer.
/// This updates the render world with the extracted ECS data of the current frame.
/// 执行渲染器的 [`ExtractSchedule`] 步骤.这会使用当前帧提取的 ECS 数据更新渲染世界
fn extract(main_world: &mut World, render_world: &mut World) {
    // temporarily add the app world to the render world as a resource
    // 临时将应用世界作为资源添加到渲染世界
    let scratch_world = main_world.remove_resource::<ScratchMainWorld>().unwrap();
    let inserted_world = core::mem::replace(main_world, scratch_world.0);
    render_world.insert_resource(MainWorld(inserted_world));
    render_world.run_schedule(ExtractSchedule);

    // move the app world back, as if nothing happened.
    // 将应用世界移回,就像什么都没发生一样
    let inserted_world = render_world.remove_resource::<MainWorld>().unwrap();
    let scratch_world = core::mem::replace(main_world, inserted_world.0);
    main_world.insert_resource(ScratchMainWorld(scratch_world));
}

/// # Safety
/// This function must be called from the main thread.
/// # 安全
/// 此函数必须从主线程调用
unsafe fn initialize_render_app(app: &mut App) {
    app.init_resource::<ScratchMainWorld>();

    let mut render_app = SubApp::new();
    render_app.update_schedule = Some(Render.intern());

    let mut extract_schedule = Schedule::new(ExtractSchedule);
    // We skip applying any commands during the ExtractSchedule
    // so commands can be applied on the render thread.
    // 我们在 ExtractSchedule 期间跳过应用任何命令,以便可以在渲染线程上应用命令
    extract_schedule.set_build_settings(ScheduleBuildSettings {
        auto_insert_apply_deferred: false,
        ..default()
    });
    extract_schedule.set_apply_final_deferred(false);

    render_app
        .add_schedule(extract_schedule)
        .add_schedule(Render::base_schedule())
        .init_resource::<renderer::PendingCommandBuffers>()
        .insert_resource(app.world().resource::<AssetServer>().clone())
        .add_systems(ExtractSchedule, PipelineCache::extract_shaders)
        .add_systems(
            Render,
            (
                // This set applies the commands from the extract schedule while the render schedule
                // is running in parallel with the main app.
                // 此集合在渲染调度与主应用并行运行时应用来自提取调度的命令
                apply_extract_commands.in_set(RenderSystems::ExtractCommands),
                (PipelineCache::process_pipeline_queue_system, render_system)
                    .chain()
                    .in_set(RenderSystems::Render),
                despawn_temporary_render_entities.in_set(RenderSystems::PostCleanup),
            ),
        );
    // 配置渲染子应用的调度和系统

    // We want the closure to have a flag to only run the RenderStartup schedule once, but the only
    // way to have the closure store this flag is by capturing it. This variable is otherwise
    // unused.
    // 我们希望闭包有一个标志,只运行一次 RenderStartup 调度,但让闭包存储此标志的唯一方法是捕获它.此变量在其他情况下未使用
    let mut should_run_startup = true;
    render_app.set_extract(move |main_world, render_world| {
        if should_run_startup {
            // Run the `RenderStartup` if it hasn't run yet. This does mean `RenderStartup` blocks
            // the rest of the app extraction, but this is necessary since extraction itself can
            // depend on resources initialized in `RenderStartup`.
            // 如果 `RenderStartup` 尚未运行,则运行它.这确实意味着 `RenderStartup` 会阻止应用的其余提取,
            // 但这是必要的,因为提取本身可能依赖于在 `RenderStartup` 中初始化的资源
            render_world.run_schedule(RenderStartup);
            should_run_startup = false;
        }

        {
            #[cfg(feature = "trace")]
            let _stage_span = tracing::info_span!("entity_sync").entered();
            entity_sync_system(main_world, render_world);
        }
        // 同步实体

        // run extract schedule
        // 运行提取调度
        extract(main_world, render_world);
    });

    let (sender, receiver) = bevy_time::create_time_channels();
    render_app.insert_resource(sender);
    app.insert_resource(receiver);
    // 创建时间通道,用于主应用和渲染子应用之间的时间同步
    app.insert_sub_app(RenderApp, render_app);
}

/// Applies the commands from the extract schedule. This happens during
/// the render schedule rather than during extraction to allow the commands to run in parallel with the
/// main app when pipelined rendering is enabled.
/// 应用来自提取调度的命令.这发生在渲染调度期间而不是提取期间,以便在启用流水线渲染时允许命令与主应用并行运行
fn apply_extract_commands(render_world: &mut World) {
    render_world.resource_scope(|render_world, mut schedules: Mut<Schedules>| {
        schedules
            .get_mut(ExtractSchedule)
            .unwrap()
            .apply_deferred(render_world);
    });
}

/// If the [`RenderAdapterInfo`] is a Qualcomm Adreno, returns its model number.
///
/// This lets us work around hardware bugs.
/// 如果 [`RenderAdapterInfo`] 是 Qualcomm Adreno,返回其型号.这让我们可以解决硬件漏洞
pub fn get_adreno_model(adapter_info: &RenderAdapterInfo) -> Option<u32> {
    if !cfg!(target_os = "android") {
        return None;
    }

    let adreno_model = adapter_info.name.strip_prefix("Adreno (TM) ")?;

    // Take suffixes into account (like Adreno 642L).
    // 考虑后缀(如 Adreno 642L)
    Some(
        adreno_model
            .chars()
            .map_while(|c| c.to_digit(10))
            .fold(0, |acc, digit| acc * 10 + digit),
    )
}

/// Get the Mali driver version if the adapter is a Mali GPU.
/// 如果适配器是 Mali GPU,获取 Mali 驱动程序版本
pub fn get_mali_driver_version(adapter_info: &RenderAdapterInfo) -> Option<u32> {
    if !cfg!(target_os = "android") {
        return None;
    }

    if !adapter_info.name.contains("Mali") {
        return None;
    }
    let driver_info = &adapter_info.driver_info;
    if let Some(start_pos) = driver_info.find("v1.r")
        && let Some(end_pos) = driver_info[start_pos..].find('p')
    {
        let start_idx = start_pos + 4; // Skip "v1.r"
        let end_idx = start_pos + end_pos;

        return driver_info[start_idx..end_idx].parse::<u32>().ok();
    }

    None
}
