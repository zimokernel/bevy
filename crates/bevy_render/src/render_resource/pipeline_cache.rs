use bevy_material::descriptor::{
    BindGroupLayoutDescriptor, CachedComputePipelineId, CachedRenderPipelineId,
    ComputePipelineDescriptor, PipelineDescriptor, RenderPipelineDescriptor,
};

use crate::{
    render_resource::*,
    renderer::{RenderAdapter, RenderDevice, WgpuWrapper},
    Extract,
};
use alloc::{borrow::Cow, sync::Arc};
use bevy_asset::{AssetEvent, AssetId, Assets, Handle};
use bevy_ecs::{
    message::MessageReader,
    resource::Resource,
    system::{Res, ResMut},
};
use bevy_platform::collections::{HashMap, HashSet};
use bevy_shader::{
    CachedPipelineId, Shader, ShaderCache, ShaderCacheError, ShaderCacheSource, ShaderDefVal,
    ValidateShader,
};
use bevy_tasks::Task;
use bevy_utils::default;
use core::{future::Future, mem};
use std::sync::{Mutex, PoisonError};
use tracing::error;
use wgpu::{PipelineCompilationOptions, VertexBufferLayout as RawVertexBufferLayout};

/// 管线枚举 - 定义特定GPU任务的数据布局和着色器逻辑
///
/// 用于存储渲染管线和计算管线的异构集合
#[derive(Debug)]
pub enum Pipeline {
    RenderPipeline(RenderPipeline),
    ComputePipeline(ComputePipeline),
}

/// 缓存的管线结构体
/// 
/// 包含管线描述符和当前状态
pub struct CachedPipeline {
    pub descriptor: PipelineDescriptor,
    pub state: CachedPipelineState,
}

/// 缓存管线的状态枚举
/// 
/// 表示插入到 PipelineCache 中的管线的当前状态
#[derive(Debug)]
pub enum CachedPipelineState {
    /// 管线GPU对象已排队等待创建
    Queued,
    /// 管线GPU对象正在创建中（异步任务）
    Creating(Task<Result<Pipeline, ShaderCacheError>>),
    /// 管线GPU对象已成功创建并可用（已在GPU上分配）
    Ok(Pipeline),
    /// 创建管线GPU对象时发生错误
    Err(ShaderCacheError),
}

impl CachedPipelineState {
    /// Convenience method to "unwrap" a pipeline state into its underlying GPU object.
    ///
    /// # Returns
    ///
    /// The method returns the allocated pipeline GPU object.
    ///
    /// # Panics
    ///
    /// This method panics if the pipeline GPU object is not available, either because it is
    /// pending creation or because an error occurred while attempting to create GPU object.
    pub fn unwrap(&self) -> &Pipeline {
        match self {
            CachedPipelineState::Ok(pipeline) => pipeline,
            CachedPipelineState::Queued => {
                panic!("Pipeline has not been compiled yet. It is still in the 'Queued' state.")
            }
            CachedPipelineState::Creating(..) => {
                panic!("Pipeline has not been compiled yet. It is still in the 'Creating' state.")
            }
            CachedPipelineState::Err(err) => panic!("{}", err),
        }
    }
}

/// 管线布局缓存的键类型
/// 
/// 由绑定组布局ID列表和推送常量范围组成
type LayoutCacheKey = (Vec<BindGroupLayoutId>, Vec<PushConstantRange>);

/// 管线布局缓存
/// 
/// 用于缓存和重用 PipelineLayout，避免重复创建相同的布局
#[derive(Default)]
struct LayoutCache {
    layouts: HashMap<LayoutCacheKey, Arc<WgpuWrapper<PipelineLayout>>>,
}

impl LayoutCache {
    /// 获取或创建管线布局
    /// 
    /// 该方法实现了管线布局的缓存机制：
    /// 1. 首先根据绑定组布局ID和推送常量范围查找缓存
    /// 2. 如果找到，直接返回缓存的布局
    /// 3. 如果未找到，创建新的 PipelineLayout 并缓存
    /// 
    /// 参数：
    /// - render_device: 渲染设备引用
    /// - bind_group_layouts: 绑定组布局数组
    /// - push_constant_ranges: 推送常量范围
    /// 
    /// 返回：
    /// - Arc<WgpuWrapper<PipelineLayout>>: 管线布局的原子引用计数指针
    fn get(
        &mut self,
        render_device: &RenderDevice,
        bind_group_layouts: &[BindGroupLayout],
        push_constant_ranges: Vec<PushConstantRange>,
    ) -> Arc<WgpuWrapper<PipelineLayout>> {
        let bind_group_ids = bind_group_layouts.iter().map(BindGroupLayout::id).collect();
        self.layouts
            .entry((bind_group_ids, push_constant_ranges))
            .or_insert_with_key(|(_, push_constant_ranges)| {
                let bind_group_layouts = bind_group_layouts
                    .iter()
                    .map(BindGroupLayout::value)
                    .collect::<Vec<_>>();
                Arc::new(WgpuWrapper::new(render_device.create_pipeline_layout(
                    &PipelineLayoutDescriptor {
                        bind_group_layouts: &bind_group_layouts,
                        push_constant_ranges,
                        ..default()
                    },
                )))
            })
            .clone()
    }
}

/// 加载并编译着色器模块
/// 
/// 该函数负责：
/// 1. 将不同格式的着色器源（SPIR-V、WGSL、Naga）转换为统一格式
/// 2. 创建着色器模块（可选验证）
/// 3. 捕获并返回着色器编译错误
/// 
/// 参数：
/// - render_device: 渲染设备引用
/// - shader_source: 着色器源（支持多种格式）
/// - validate_shader: 是否启用着色器验证
/// 
/// 返回：
/// - Result<WgpuWrapper<ShaderModule>, ShaderCacheError>: 着色器模块或错误
fn load_module(
    render_device: &RenderDevice,
    shader_source: ShaderCacheSource,
    validate_shader: &ValidateShader,
) -> Result<WgpuWrapper<ShaderModule>, ShaderCacheError> {
    // 转换着色器源格式
    let shader_source = match shader_source {
        #[cfg(feature = "shader_format_spirv")]
        ShaderCacheSource::SpirV(data) => wgpu::util::make_spirv(data),
        #[cfg(not(feature = "shader_format_spirv"))]
        ShaderCacheSource::SpirV(_) => {
            unimplemented!("Enable feature \"shader_format_spirv\" to use SPIR-V shaders")
        }
        ShaderCacheSource::Wgsl(src) => ShaderSource::Wgsl(Cow::Owned(src)),
        #[cfg(not(feature = "decoupled_naga"))]
        ShaderCacheSource::Naga(src) => ShaderSource::Naga(Cow::Owned(src)),
    };
    
    let module_descriptor = ShaderModuleDescriptor {
        label: None,
        source: shader_source,
    };

    // 启用错误捕获范围
    render_device
        .wgpu_device()
        .push_error_scope(wgpu::ErrorFilter::Validation);

    // 创建着色器模块（根据验证选项选择不同路径）
    let shader_module = WgpuWrapper::new(match validate_shader {
        ValidateShader::Enabled => {
            render_device.create_and_validate_shader_module(module_descriptor)
        }
        // 禁用验证时使用不安全的快速路径（性能优化）
        ValidateShader::Disabled => unsafe {
            render_device.create_shader_module(module_descriptor)
        },
    });

    // 检查并返回错误
    let error = render_device.wgpu_device().pop_error_scope();

    // 在原生平台上立即捕获错误，在 WASM 上可能需要更长时间
    if let Some(Some(wgpu::Error::Validation { description, .. })) =
        bevy_tasks::futures::now_or_never(error)
    {
        return Err(ShaderCacheError::CreateShaderModule(description));
    }

    Ok(shader_module)
}

/// 绑定组布局缓存
/// 
/// 用于缓存和重用 BindGroupLayout，避免重复创建相同的绑定组布局
#[derive(Default)]
struct BindGroupLayoutCache {
    bgls: HashMap<BindGroupLayoutDescriptor, BindGroupLayout>,
}

impl BindGroupLayoutCache {
    /// 获取或创建绑定组布局
    /// 
    /// 该方法实现了绑定组布局的缓存机制：
    /// 1. 首先根据 BindGroupLayoutDescriptor 查找缓存
    /// 2. 如果找到，直接返回缓存的布局
    /// 3. 如果未找到，创建新的 BindGroupLayout 并缓存
    /// 
    /// 参数：
    /// - render_device: 渲染设备引用
    /// - descriptor: 绑定组布局描述符
    /// 
    /// 返回：
    /// - BindGroupLayout: 绑定组布局
    fn get(
        &mut self,
        render_device: &RenderDevice,
        descriptor: BindGroupLayoutDescriptor,
    ) -> BindGroupLayout {
        self.bgls
            .entry(descriptor)
            .or_insert_with_key(|descriptor| {
                render_device
                    .create_bind_group_layout(descriptor.label.as_ref(), &descriptor.entries)
            })
            .clone()
    }
}

/// Cache for render and compute pipelines.
///
/// The cache stores existing render and compute pipelines allocated on the GPU, as well as
/// pending creation. Pipelines inserted into the cache are identified by a unique ID, which
/// can be used to retrieve the actual GPU object once it's ready. The creation of the GPU
/// pipeline object is deferred to the [`RenderSystems::Render`] step, just before the render
/// graph starts being processed, as this requires access to the GPU.
///
/// Note that the cache does not perform automatic deduplication of identical pipelines. It is
/// up to the user not to insert the same pipeline twice to avoid wasting GPU resources.
///
/// [`RenderSystems::Render`]: crate::RenderSystems::Render
#[derive(Resource)]
/// 渲染管线和计算管线的缓存管理器
/// 
/// 该缓存负责：
/// 1. 存储已创建的 GPU 管线对象（渲染管线和计算管线）
/// 2. 管理待创建的管线队列
/// 3. 缓存和重用着色器模块、管线布局和绑定组布局
/// 4. 支持异步管线编译（避免阻塞主线程）
/// 
/// 管线创建流程：
/// 1. 调用 queue_render_pipeline 或 queue_compute_pipeline 将管线加入队列
/// 2. 在 RenderSystems::Render 阶段之前，prepare_pipelines 会处理所有排队的管线
/// 3. 管线会经历 Queued -> Creating -> Ok/Err 的状态转换
/// 4. 完成后可以通过 get_render_pipeline 或 get_compute_pipeline 获取 GPU 管线
/// 
/// 注意：该缓存不会自动对相同的管线进行去重，用户需要避免重复插入相同的管线
/// 以避免浪费 GPU 资源。
#[derive(Resource)]
pub struct PipelineCache {
    /// 管线布局缓存（线程安全）
    layout_cache: Arc<Mutex<LayoutCache>>,
    /// 绑定组布局缓存（线程安全）
    bindgroup_layout_cache: Arc<Mutex<BindGroupLayoutCache>>,
    /// 着色器缓存（线程安全）
    shader_cache: Arc<Mutex<ShaderCache<WgpuWrapper<ShaderModule>, RenderDevice>>>,
    /// 渲染设备引用
    device: RenderDevice,
    /// 所有已缓存的管线
    pipelines: Vec<CachedPipeline>,
    /// 等待中的管线 ID 集合
    waiting_pipelines: HashSet<CachedPipelineId>,
    /// 新加入的管线队列（线程安全）
    new_pipelines: Mutex<Vec<CachedPipeline>>,
    /// 全局着色器定义（影响所有着色器编译）
    global_shader_defs: Vec<ShaderDefVal>,
    /// 如果为 true，禁用异步管线编译
    /// 在 macOS、wasm 或没有 multi_threaded 特性时无效
    synchronous_pipeline_compilation: bool,
}

impl PipelineCache {
    /// Returns an iterator over the pipelines in the pipeline cache.
    pub fn pipelines(&self) -> impl Iterator<Item = &CachedPipeline> {
        self.pipelines.iter()
    }

    /// Returns a iterator of the IDs of all currently waiting pipelines.
    pub fn waiting_pipelines(&self) -> impl Iterator<Item = CachedPipelineId> + '_ {
        self.waiting_pipelines.iter().copied()
    }

    /// Create a new pipeline cache associated with the given render device.
    /// 创建新的管线缓存
    /// 
    /// 该方法会：
    /// 1. 初始化全局着色器定义（根据平台特性）
    /// 2. 创建着色器缓存
    /// 3. 初始化所有内部缓存结构
    /// 
    /// 参数：
    /// - device: 渲染设备
    /// - render_adapter: 渲染适配器
    /// - synchronous_pipeline_compilation: 是否同步编译管线
    /// 
    /// 返回：
    /// - Self: 新创建的 PipelineCache 实例
    pub fn new(
        device: RenderDevice,
        render_adapter: RenderAdapter,
        synchronous_pipeline_compilation: bool,
    ) -> Self {
        let mut global_shader_defs = Vec::new();
        
        // WebGL 平台特定的全局着色器定义
        #[cfg(all(feature = "webgl", target_arch = "wasm32", not(feature = "webgpu")))]
        {
            global_shader_defs.push("NO_ARRAY_TEXTURES_SUPPORT".into());
            global_shader_defs.push("NO_CUBE_ARRAY_TEXTURES_SUPPORT".into());
            global_shader_defs.push("SIXTEEN_BYTE_ALIGNMENT".into());
        }

        // 模拟器平台特定的全局着色器定义
        if cfg!(target_abi = "sim") {
            global_shader_defs.push("NO_CUBE_ARRAY_TEXTURES_SUPPORT".into());
        }

        // 添加存储缓冲区绑定数量的全局定义
        global_shader_defs.push(ShaderDefVal::UInt(
            String::from("AVAILABLE_STORAGE_BUFFER_BINDINGS"),
            device.limits().max_storage_buffers_per_shader_stage,
        ));

        Self {
            // 初始化着色器缓存（线程安全）
            shader_cache: Arc::new(Mutex::new(ShaderCache::new(
                device.features(),
                render_adapter.get_downlevel_capabilities().flags,
                load_module,
            ))),
            device,
            layout_cache: default(),
            bindgroup_layout_cache: default(),
            waiting_pipelines: default(),
            new_pipelines: default(),
            pipelines: default(),
            global_shader_defs,
            synchronous_pipeline_compilation,
        }
    }

    /// Get the state of a cached render pipeline.
    ///
    /// See [`PipelineCache::queue_render_pipeline()`].
    /// 获取缓存的渲染管线状态
    /// 
    /// 如果管线 ID 不在 `pipelines` 中，则说明它在 `new_pipelines` 中排队
    /// 
    /// 参数：
    /// - id: 缓存的渲染管线 ID
    /// 
    /// 返回：
    /// - &CachedPipelineState: 管线状态引用
    #[inline]
    pub fn get_render_pipeline_state(&self, id: CachedRenderPipelineId) -> &CachedPipelineState {
        // 如果管线 ID 不在 `pipelines` 中，则说明它在 `new_pipelines` 中排队
        self.pipelines
            .get(id.id())
            .map_or(&CachedPipelineState::Queued, |pipeline| &pipeline.state)
    }

    /// Get the state of a cached compute pipeline.
    ///
    /// See [`PipelineCache::queue_compute_pipeline()`].
    #[inline]
    pub fn get_compute_pipeline_state(&self, id: CachedComputePipelineId) -> &CachedPipelineState {
        // If the pipeline id isn't in `pipelines`, it's queued in `new_pipelines`
        self.pipelines
            .get(id.id())
            .map_or(&CachedPipelineState::Queued, |pipeline| &pipeline.state)
    }

    /// Get the render pipeline descriptor a cached render pipeline was inserted from.
    ///
    /// See [`PipelineCache::queue_render_pipeline()`].
    ///
    /// **Note**: Be careful calling this method. It will panic if called with a pipeline that
    /// has been queued but has not yet been processed by [`PipelineCache::process_queue()`].
    #[inline]
    pub fn get_render_pipeline_descriptor(
        &self,
        id: CachedRenderPipelineId,
    ) -> &RenderPipelineDescriptor {
        match &self.pipelines[id.id()].descriptor {
            PipelineDescriptor::RenderPipelineDescriptor(descriptor) => descriptor,
            PipelineDescriptor::ComputePipelineDescriptor(_) => unreachable!(),
        }
    }

    /// Get the compute pipeline descriptor a cached render pipeline was inserted from.
    ///
    /// See [`PipelineCache::queue_compute_pipeline()`].
    ///
    /// **Note**: Be careful calling this method. It will panic if called with a pipeline that
    /// has been queued but has not yet been processed by [`PipelineCache::process_queue()`].
    #[inline]
    pub fn get_compute_pipeline_descriptor(
        &self,
        id: CachedComputePipelineId,
    ) -> &ComputePipelineDescriptor {
        match &self.pipelines[id.id()].descriptor {
            PipelineDescriptor::RenderPipelineDescriptor(_) => unreachable!(),
            PipelineDescriptor::ComputePipelineDescriptor(descriptor) => descriptor,
        }
    }

    /// Try to retrieve a render pipeline GPU object from a cached ID.
    ///
    /// # Returns
    ///
    /// This method returns a successfully created render pipeline if any, or `None` if the pipeline
    /// was not created yet or if there was an error during creation. You can check the actual creation
    /// state with [`PipelineCache::get_render_pipeline_state()`].
    #[inline]
    pub fn get_render_pipeline(&self, id: CachedRenderPipelineId) -> Option<&RenderPipeline> {
        if let CachedPipelineState::Ok(Pipeline::RenderPipeline(pipeline)) =
            &self.pipelines.get(id.id())?.state
        {
            Some(pipeline)
        } else {
            None
        }
    }

    /// Wait for a render pipeline to finish compiling.
    #[inline]
    pub fn block_on_render_pipeline(&mut self, id: CachedRenderPipelineId) {
        if self.pipelines.len() <= id.id() {
            self.process_queue();
        }

        let state = &mut self.pipelines[id.id()].state;
        if let CachedPipelineState::Creating(task) = state {
            *state = match bevy_tasks::block_on(task) {
                Ok(p) => CachedPipelineState::Ok(p),
                Err(e) => CachedPipelineState::Err(e),
            };
        }
    }

    /// Try to retrieve a compute pipeline GPU object from a cached ID.
    ///
    /// # Returns
    ///
    /// This method returns a successfully created compute pipeline if any, or `None` if the pipeline
    /// was not created yet or if there was an error during creation. You can check the actual creation
    /// state with [`PipelineCache::get_compute_pipeline_state()`].
    #[inline]
    pub fn get_compute_pipeline(&self, id: CachedComputePipelineId) -> Option<&ComputePipeline> {
        if let CachedPipelineState::Ok(Pipeline::ComputePipeline(pipeline)) =
            &self.pipelines.get(id.id())?.state
        {
            Some(pipeline)
        } else {
            None
        }
    }

    /// Insert a render pipeline into the cache, and queue its creation.
    ///
    /// The pipeline is always inserted and queued for creation. There is no attempt to deduplicate it with
    /// an already cached pipeline.
    ///
    /// # Returns
    ///
    /// This method returns the unique render shader ID of the cached pipeline, which can be used to query
    /// the caching state with [`get_render_pipeline_state()`] and to retrieve the created GPU pipeline once
    /// it's ready with [`get_render_pipeline()`].
    ///
    /// [`get_render_pipeline_state()`]: PipelineCache::get_render_pipeline_state
    /// [`get_render_pipeline()`]: PipelineCache::get_render_pipeline
    /// 将渲染管线插入缓存并排队等待创建
    /// 
    /// 该方法会：
    /// 1. 为新管线分配唯一 ID
    /// 2. 将管线描述符和 Queued 状态添加到新管线队列
    /// 3. 返回管线 ID，用于后续查询管线状态或获取 GPU 管线
    /// 
    /// 注意：该方法不会尝试对已缓存的管线进行去重，即使插入相同的管线
    /// 也会创建新的缓存条目。
    /// 
    /// 参数：
    /// - descriptor: 渲染管线描述符
    /// 
    /// 返回：
    /// - CachedRenderPipelineId: 缓存的渲染管线 ID
    pub fn queue_render_pipeline(
        &self,
        descriptor: RenderPipelineDescriptor,
    ) -> CachedRenderPipelineId {
        // 锁定新管线队列（线程安全）
        let mut new_pipelines = self
            .new_pipelines
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        // 生成唯一 ID
        let id = CachedRenderPipelineId::new(self.pipelines.len() + new_pipelines.len());
        // 将管线添加到队列
        new_pipelines.push(CachedPipeline {
            descriptor: PipelineDescriptor::RenderPipelineDescriptor(Box::new(descriptor)),
            state: CachedPipelineState::Queued,
        });
        id
    }

    /// Insert a compute pipeline into the cache, and queue its creation.
    ///
    /// The pipeline is always inserted and queued for creation. There is no attempt to deduplicate it with
    /// an already cached pipeline.
    ///
    /// # Returns
    ///
    /// This method returns the unique compute shader ID of the cached pipeline, which can be used to query
    /// the caching state with [`get_compute_pipeline_state()`] and to retrieve the created GPU pipeline once
    /// it's ready with [`get_compute_pipeline()`].
    ///
    /// [`get_compute_pipeline_state()`]: PipelineCache::get_compute_pipeline_state
    /// [`get_compute_pipeline()`]: PipelineCache::get_compute_pipeline
    /// 将计算管线插入缓存并排队等待创建
    /// 
    /// 该方法会：
    /// 1. 为新计算管线分配唯一 ID
    /// 2. 将计算管线描述符和 Queued 状态添加到新管线队列
    /// 3. 返回管线 ID，用于后续查询管线状态或获取 GPU 计算管线
    /// 
    /// 注意：该方法不会尝试对已缓存的管线进行去重，即使插入相同的管线
    /// 也会创建新的缓存条目。
    /// 
    /// 参数：
    /// - descriptor: 计算管线描述符
    /// 
    /// 返回：
    /// - CachedComputePipelineId: 缓存的计算管线 ID
    pub fn queue_compute_pipeline(
        &self,
        descriptor: ComputePipelineDescriptor,
    ) -> CachedComputePipelineId {
        // 锁定新管线队列（线程安全）
        let mut new_pipelines = self
            .new_pipelines
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        // 生成唯一 ID
        let id = CachedComputePipelineId::new(self.pipelines.len() + new_pipelines.len());
        // 将计算管线添加到队列
        new_pipelines.push(CachedPipeline {
            descriptor: PipelineDescriptor::ComputePipelineDescriptor(Box::new(descriptor)),
            state: CachedPipelineState::Queued,
        });
        id
    }

    pub fn get_bind_group_layout(
        &self,
        bind_group_layout_descriptor: &BindGroupLayoutDescriptor,
    ) -> BindGroupLayout {
        self.bindgroup_layout_cache
            .lock()
            .unwrap()
            .get(&self.device, bind_group_layout_descriptor.clone())
    }

    fn set_shader(&mut self, id: AssetId<Shader>, shader: Shader) {
        let mut shader_cache = self.shader_cache.lock().unwrap();
        let pipelines_to_queue = shader_cache.set_shader(id, shader);
        for cached_pipeline in pipelines_to_queue {
            self.pipelines[cached_pipeline].state = CachedPipelineState::Queued;
            self.waiting_pipelines.insert(cached_pipeline);
        }
    }

    fn remove_shader(&mut self, shader: AssetId<Shader>) {
        let mut shader_cache = self.shader_cache.lock().unwrap();
        let pipelines_to_queue = shader_cache.remove(shader);
        for cached_pipeline in pipelines_to_queue {
            self.pipelines[cached_pipeline].state = CachedPipelineState::Queued;
            self.waiting_pipelines.insert(cached_pipeline);
        }
    }

    fn start_create_render_pipeline(
        &mut self,
        id: CachedPipelineId,
        descriptor: RenderPipelineDescriptor,
    ) -> CachedPipelineState {
        let device = self.device.clone();
        let shader_cache = self.shader_cache.clone();
        let layout_cache = self.layout_cache.clone();
        let mut bindgroup_layout_cache = self.bindgroup_layout_cache.lock().unwrap();
        let bind_group_layout = descriptor
            .layout
            .iter()
            .map(|bind_group_layout_descriptor| {
                bindgroup_layout_cache.get(&self.device, bind_group_layout_descriptor.clone())
            })
            .collect::<Vec<_>>();

        create_pipeline_task(
            async move {
                let mut shader_cache = shader_cache.lock().unwrap();
                let mut layout_cache = layout_cache.lock().unwrap();

                let vertex_module = match shader_cache.get(
                    &device,
                    id,
                    descriptor.vertex.shader.id(),
                    &descriptor.vertex.shader_defs,
                ) {
                    Ok(module) => module,
                    Err(err) => return Err(err),
                };

                let fragment_module = match &descriptor.fragment {
                    Some(fragment) => {
                        match shader_cache.get(
                            &device,
                            id,
                            fragment.shader.id(),
                            &fragment.shader_defs,
                        ) {
                            Ok(module) => Some(module),
                            Err(err) => return Err(err),
                        }
                    }
                    None => None,
                };

                let layout =
                    if descriptor.layout.is_empty() && descriptor.push_constant_ranges.is_empty() {
                        None
                    } else {
                        Some(layout_cache.get(
                            &device,
                            &bind_group_layout,
                            descriptor.push_constant_ranges.to_vec(),
                        ))
                    };

                drop((shader_cache, layout_cache));

                let vertex_buffer_layouts = descriptor
                    .vertex
                    .buffers
                    .iter()
                    .map(|layout| RawVertexBufferLayout {
                        array_stride: layout.array_stride,
                        attributes: &layout.attributes,
                        step_mode: layout.step_mode,
                    })
                    .collect::<Vec<_>>();

                let fragment_data = descriptor.fragment.as_ref().map(|fragment| {
                    (
                        fragment_module.unwrap(),
                        fragment.entry_point.as_deref(),
                        fragment.targets.as_slice(),
                    )
                });

                // TODO: Expose the rest of this somehow
                let compilation_options = PipelineCompilationOptions {
                    constants: &[],
                    zero_initialize_workgroup_memory: descriptor.zero_initialize_workgroup_memory,
                };

                let descriptor = RawRenderPipelineDescriptor {
                    multiview: None,
                    depth_stencil: descriptor.depth_stencil.clone(),
                    label: descriptor.label.as_deref(),
                    layout: layout.as_ref().map(|layout| -> &PipelineLayout { layout }),
                    multisample: descriptor.multisample,
                    primitive: descriptor.primitive,
                    vertex: RawVertexState {
                        buffers: &vertex_buffer_layouts,
                        entry_point: descriptor.vertex.entry_point.as_deref(),
                        module: &vertex_module,
                        // TODO: Should this be the same as the fragment compilation options?
                        compilation_options: compilation_options.clone(),
                    },
                    fragment: fragment_data
                        .as_ref()
                        .map(|(module, entry_point, targets)| RawFragmentState {
                            entry_point: entry_point.as_deref(),
                            module,
                            targets,
                            // TODO: Should this be the same as the vertex compilation options?
                            compilation_options,
                        }),
                    cache: None,
                };

                Ok(Pipeline::RenderPipeline(
                    device.create_render_pipeline(&descriptor),
                ))
            },
            self.synchronous_pipeline_compilation,
        )
    }

    fn start_create_compute_pipeline(
        &mut self,
        id: CachedPipelineId,
        descriptor: ComputePipelineDescriptor,
    ) -> CachedPipelineState {
        let device = self.device.clone();
        let shader_cache = self.shader_cache.clone();
        let layout_cache = self.layout_cache.clone();
        let mut bindgroup_layout_cache = self.bindgroup_layout_cache.lock().unwrap();
        let bind_group_layout = descriptor
            .layout
            .iter()
            .map(|bind_group_layout_descriptor| {
                bindgroup_layout_cache.get(&self.device, bind_group_layout_descriptor.clone())
            })
            .collect::<Vec<_>>();

        create_pipeline_task(
            async move {
                let mut shader_cache = shader_cache.lock().unwrap();
                let mut layout_cache = layout_cache.lock().unwrap();

                let compute_module = match shader_cache.get(
                    &device,
                    id,
                    descriptor.shader.id(),
                    &descriptor.shader_defs,
                ) {
                    Ok(module) => module,
                    Err(err) => return Err(err),
                };

                let layout =
                    if descriptor.layout.is_empty() && descriptor.push_constant_ranges.is_empty() {
                        None
                    } else {
                        Some(layout_cache.get(
                            &device,
                            &bind_group_layout,
                            descriptor.push_constant_ranges.to_vec(),
                        ))
                    };

                drop((shader_cache, layout_cache));

                let descriptor = RawComputePipelineDescriptor {
                    label: descriptor.label.as_deref(),
                    layout: layout.as_ref().map(|layout| -> &PipelineLayout { layout }),
                    module: &compute_module,
                    entry_point: descriptor.entry_point.as_deref(),
                    // TODO: Expose the rest of this somehow
                    compilation_options: PipelineCompilationOptions {
                        constants: &[],
                        zero_initialize_workgroup_memory: descriptor
                            .zero_initialize_workgroup_memory,
                    },
                    cache: None,
                };

                Ok(Pipeline::ComputePipeline(
                    device.create_compute_pipeline(&descriptor),
                ))
            },
            self.synchronous_pipeline_compilation,
        )
    }

    /// Process the pipeline queue and create all pending pipelines if possible.
    ///
    /// This is generally called automatically during the [`RenderSystems::Render`] step, but can
    /// be called manually to force creation at a different time.
    ///
    /// [`RenderSystems::Render`]: crate::RenderSystems::Render
    pub fn process_queue(&mut self) {
        let mut waiting_pipelines = mem::take(&mut self.waiting_pipelines);
        let mut pipelines = mem::take(&mut self.pipelines);

        {
            let mut new_pipelines = self
                .new_pipelines
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            for new_pipeline in new_pipelines.drain(..) {
                let id = pipelines.len();
                pipelines.push(new_pipeline);
                waiting_pipelines.insert(id);
            }
        }

        for id in waiting_pipelines {
            self.process_pipeline(&mut pipelines[id], id);
        }

        self.pipelines = pipelines;
    }

    fn process_pipeline(&mut self, cached_pipeline: &mut CachedPipeline, id: usize) {
        match &mut cached_pipeline.state {
            CachedPipelineState::Queued => {
                cached_pipeline.state = match &cached_pipeline.descriptor {
                    PipelineDescriptor::RenderPipelineDescriptor(descriptor) => {
                        self.start_create_render_pipeline(id, *descriptor.clone())
                    }
                    PipelineDescriptor::ComputePipelineDescriptor(descriptor) => {
                        self.start_create_compute_pipeline(id, *descriptor.clone())
                    }
                };
            }

            CachedPipelineState::Creating(task) => match bevy_tasks::futures::check_ready(task) {
                Some(Ok(pipeline)) => {
                    cached_pipeline.state = CachedPipelineState::Ok(pipeline);
                    return;
                }
                Some(Err(err)) => cached_pipeline.state = CachedPipelineState::Err(err),
                _ => (),
            },

            CachedPipelineState::Err(err) => match err {
                // Retry
                ShaderCacheError::ShaderNotLoaded(_)
                | ShaderCacheError::ShaderImportNotYetAvailable => {
                    cached_pipeline.state = CachedPipelineState::Queued;
                }

                // Shader could not be processed ... retrying won't help
                ShaderCacheError::ProcessShaderError(err) => {
                    let error_detail =
                        err.emit_to_string(&self.shader_cache.lock().unwrap().composer);
                    if std::env::var("VERBOSE_SHADER_ERROR")
                        .is_ok_and(|v| !(v.is_empty() || v == "0" || v == "false"))
                    {
                        error!("{}", pipeline_error_context(cached_pipeline));
                    }
                    error!("failed to process shader error:\n{}", error_detail);
                    return;
                }
                ShaderCacheError::CreateShaderModule(description) => {
                    error!("failed to create shader module: {}", description);
                    return;
                }
            },

            CachedPipelineState::Ok(_) => return,
        }

        // Retry
        self.waiting_pipelines.insert(id);
    }

    pub(crate) fn process_pipeline_queue_system(mut cache: ResMut<Self>) {
        cache.process_queue();
    }

    pub(crate) fn extract_shaders(
        mut cache: ResMut<Self>,
        shaders: Extract<Res<Assets<Shader>>>,
        mut events: Extract<MessageReader<AssetEvent<Shader>>>,
    ) {
        for event in events.read() {
            #[expect(
                clippy::match_same_arms,
                reason = "LoadedWithDependencies is marked as a TODO, so it's likely this will no longer lint soon."
            )]
            match event {
                // PERF: Instead of blocking waiting for the shader cache lock, try again next frame if the lock is currently held
                AssetEvent::Added { id } | AssetEvent::Modified { id } => {
                    if let Some(shader) = shaders.get(*id) {
                        let mut shader = shader.clone();
                        shader.shader_defs.extend(cache.global_shader_defs.clone());

                        cache.set_shader(*id, shader);
                    }
                }
                AssetEvent::Removed { id } => cache.remove_shader(*id),
                AssetEvent::Unused { .. } => {}
                AssetEvent::LoadedWithDependencies { .. } => {
                    // TODO: handle this
                }
            }
        }
    }
}

fn pipeline_error_context(cached_pipeline: &CachedPipeline) -> String {
    fn format(
        shader: &Handle<Shader>,
        entry: &Option<Cow<'static, str>>,
        shader_defs: &[ShaderDefVal],
    ) -> String {
        let source = match shader.path() {
            Some(path) => path.path().to_string_lossy().to_string(),
            None => String::new(),
        };
        let entry = match entry {
            Some(entry) => entry.to_string(),
            None => String::new(),
        };
        let shader_defs = shader_defs
            .iter()
            .flat_map(|def| match def {
                ShaderDefVal::Bool(k, v) if *v => Some(k.to_string()),
                ShaderDefVal::Int(k, v) => Some(format!("{k} = {v}")),
                ShaderDefVal::UInt(k, v) => Some(format!("{k} = {v}")),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!("{source}:{entry}\nshader defs: {shader_defs}")
    }
    match &cached_pipeline.descriptor {
        PipelineDescriptor::RenderPipelineDescriptor(desc) => {
            let vert = &desc.vertex;
            let vert_str = format(&vert.shader, &vert.entry_point, &vert.shader_defs);
            let Some(frag) = desc.fragment.as_ref() else {
                return vert_str;
            };
            let frag_str = format(&frag.shader, &frag.entry_point, &frag.shader_defs);
            format!("vertex {vert_str}\nfragment {frag_str}")
        }
        PipelineDescriptor::ComputePipelineDescriptor(desc) => {
            format(&desc.shader, &desc.entry_point, &desc.shader_defs)
        }
    }
}

#[cfg(all(
    not(target_arch = "wasm32"),
    not(target_os = "macos"),
    feature = "multi_threaded"
))]
fn create_pipeline_task(
    task: impl Future<Output = Result<Pipeline, ShaderCacheError>> + Send + 'static,
    sync: bool,
) -> CachedPipelineState {
    if !sync {
        return CachedPipelineState::Creating(bevy_tasks::AsyncComputeTaskPool::get().spawn(task));
    }

    match bevy_tasks::block_on(task) {
        Ok(pipeline) => CachedPipelineState::Ok(pipeline),
        Err(err) => CachedPipelineState::Err(err),
    }
}

#[cfg(any(
    target_arch = "wasm32",
    target_os = "macos",
    not(feature = "multi_threaded")
))]
fn create_pipeline_task(
    task: impl Future<Output = Result<Pipeline, ShaderCacheError>> + Send + 'static,
    _sync: bool,
) -> CachedPipelineState {
    match bevy_tasks::block_on(task) {
        Ok(pipeline) => CachedPipelineState::Ok(pipeline),
        Err(err) => CachedPipelineState::Err(err),
    }
}
