# bevy_material 模块总结

## 概述

`bevy_material` 是 Bevy 游戏引擎的**材质系统核心模块**，提供了定义和管理渲染材质的基础设施。它是一个抽象层，允许开发者定义自定义材质，控制渲染行为，并与底层渲染管线集成。

**核心功能**：
- **材质定义**：通过 `Material` trait 和派生宏定义材质
- **透明度控制**：多种 alpha 混合模式
- **管线管理**：材质渲染管线的创建和缓存
- **绑定组**：自动生成 GPU 资源绑定
- **专业化**：基于材质属性的管线专业化

---

## 核心架构

### 材质系统流程

```
[Material Definition] → [AsBindGroup Derive] → [Material Properties] → [Pipeline Specialization] → [Render]
       ↓                      ↓                    ↓                      ↓                  ↓
  定义材质结构      生成绑定组代码      计算材质属性        创建渲染管线        GPU执行
```

**关键阶段**：
1. **定义**：使用 `#[derive(AsBindGroup)]` 定义材质结构
2. **提取**：从主世界提取材质数据到渲染世界
3. **准备**：创建 GPU 资源（缓冲区、纹理、绑定组）
4. **专业化**：根据材质属性创建或选择渲染管线
5. **渲染**：使用材质渲染网格

---

## 主要子模块

### 模块功能总览

| 模块 | 功能 | 关键组件 |
|------|------|----------|
| **alpha** | 透明度模式 | `AlphaMode` (7种混合模式) |
| **key** | 管线键管理 | `ErasedMeshPipelineKey`, `ErasedMaterialKey` |
| **descriptor** | 管线描述符 | `RenderPipelineDescriptor`, `BindGroupLayoutDescriptor` |
| **specialize** | 管线专业化 | `BaseSpecializeFn`, `PrepassSpecializeFn` |
| **labels** | 标签系统 | `DrawFunctionLabel`, `ShaderLabel` |
| **bind_group_layout_entries** | 绑定组布局 | 自动生成绑定组条目 |
| **opaque** | 不透明渲染方法 | `OpaqueRendererMethod` (前向/延迟) |
| **phase** | 渲染阶段类型 | `RenderPhaseType` |

---

## 核心子模块详解

### 1. Material Trait 和派生宏

#### Material Trait 定义

**文件**: [`bevy_pbr/src/material.rs`](file:///d:/work/ttc/bevy/crates/bevy_pbr/src/material.rs)

```rust
pub trait Material: Asset + AsBindGroup + Clone + Sized {
    // 顶点着色器（可选，默认使用网格顶点着色器）
    fn vertex_shader() -> ShaderRef {
        ShaderRef::Default
    }
    
    // 片段着色器（可选，默认使用网格片段着色器）
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Default
    }
    
    // 透明度模式
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }
    
    // 不透明渲染方法（前向或延迟）
    fn opaque_render_method(&self) -> OpaqueRendererMethod {
        OpaqueRendererMethod::Forward
    }
    
    // 深度偏移（用于避免 z-fighting）
    fn depth_bias(&self) -> f32 {
        0.0
    }
    
    // 是否读取透射纹理（用于屏幕空间透射）
    fn reads_view_transmission_texture(&self) -> bool {
        false
    }
    
    // 是否启用预渲染通道
    fn enable_prepass() -> bool {
        true
    }
    
    // 是否启用阴影
    fn enable_shadows() -> bool {
        true
    }
}
```

#### AsBindGroup 派生宏

**核心功能**：自动生成材质到 GPU 绑定组的代码

```rust
#[derive(AsBindGroup, Debug, Clone, Asset, TypePath)]
pub struct CustomMaterial {
    // Uniform 缓冲区（自动转换为 ShaderType）
    #[uniform(0)]
    color: LinearRgba,
    
    // 纹理绑定
    #[texture(1)]
    #[sampler(2)]
    color_texture: Handle<Image>,
    
    // 数组纹理
    #[texture(3, binding_array)]
    #[sampler(4, binding_array)]
    array_texture: Vec<Handle<Image>>,
    
    // Bindless 纹理
    #[texture(5, bindless)]
    bindless_texture: Handle<Image>,
}
```

**派生宏属性**：

| 属性 | 功能 | 示例 |
|------|------|------|
| `#[uniform(index)]` | Uniform 缓冲区绑定 | `#[uniform(0)] color: Vec4` |
| `#[texture(index)]` | 纹理绑定 | `#[texture(1)] tex: Handle<Image>` |
| `#[sampler(index)]` | 采样器绑定 | `#[sampler(2)] sampler: Sampler` |
| `binding_array` | 绑定数组（多个纹理） | `#[texture(3, binding_array)]` |
| `bindless` | Bindless 纹理 | `#[texture(4, bindless)]` |
| `#[dependency]` | 资源依赖 | 自动跟踪纹理加载 |
| `#[data(index)]` | 自定义数据 | 用于绑定组数据 |

**生成的代码**：

```rust
// 派生宏自动生成以下实现
impl AsBindGroup for CustomMaterial {
    type Data = CustomMaterialUniform;
    
    fn as_bind_group(
        &self,
        layout: &BindGroupLayout,
        render_device: &RenderDevice,
        images: &RenderAssets<Image>,
        fallback_image: &FallbackImage,
    ) -> Result<PreparedBindGroup<Self::Data>, AsBindGroupError> {
        // 自动生成的绑定组创建代码
    }
    
    fn bind_group_layout(
        render_device: &RenderDevice,
    ) -> BindGroupLayout
    where
        Self: Sized,
    {
        // 自动生成的绑定组布局
    }
}
```

#### 简单材质示例

```rust
use bevy::prelude::*;
use bevy_pbr::Material;
use bevy_render::render_resource::AsBindGroup;
use bevy_shader::ShaderRef;

#[derive(AsBindGroup, Debug, Clone, Asset, TypePath)]
pub struct SimpleMaterial {
    #[uniform(0)]
    pub color: Color,
    
    #[texture(1)]
    #[sampler(2)]
    pub texture: Option<Handle<Image>>,
}

impl Material for SimpleMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/simple_material.wgsl".into()
    }
    
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}
```

**对应的 WGSL 着色器**：

```wgsl
@group(3) @binding(0) var<uniform> color: vec4<f32>;
@group(3) @binding(1) var tex: texture_2d<f32>;
@group(3) @binding(2) var tex_sampler: sampler;

@fragment
fn fragment(
    @location(0) uv: vec2<f32>,
) -> @location(0) vec4<f32> {
    var sampled_color = textureSample(tex, tex_sampler, uv);
    return sampled_color * color;
}
```

---

### 2. AlphaMode（透明度模式）

**文件**: [`alpha.rs`](file:///d:/work/ttc/bevy/crates/bevy_material/src/alpha.rs)

#### AlphaMode 枚举

```rust
#[derive(Debug, Default, Reflect, Copy, Clone, PartialEq)]
pub enum AlphaMode {
    #[default]
    Opaque,              // 完全不透明
    Mask(f32),           // Alpha 测试（阈值）
    Blend,               // 标准 alpha 混合
    Premultiplied,       // 预乘 alpha
    AlphaToCoverage,     // Alpha 到覆盖（需要 MSAA）
    Add,                 // 加法混合（发光效果）
    Multiply,            // 乘法混合（染色效果）
}
```

#### 各模式详解

| 模式 | 计算公式 | 用途 | 性能 |
|------|----------|------|------|
| **Opaque** | `output = color` | 完全不透明物体 | 最快 |
| **Mask(threshold)** | `alpha < threshold ? discard : output` | 植被、粒子 | 快 |
| **Blend** | `output = src * src_alpha + dst * (1 - src_alpha)` | 半透明物体 | 中等（需要排序） |
| **Premultiplied** | `output = src + dst * (1 - src_alpha)` | 预乘 alpha 纹理 | 中等 |
| **AlphaToCoverage** | 硬件 MSAA 功能 | 高质量植被 | 中等（需要 MSAA） |
| **Add** | `output = src + dst` | 发光、能量效果 | 快（不需要排序） |
| **Multiply** | `output = src * dst` | 染色、玻璃效果 | 快（不需要排序） |

#### 使用示例

```rust
#[derive(AsBindGroup, Debug, Clone, Asset, TypePath)]
pub struct GlassMaterial {
    #[uniform(0)]
    pub color: Color,
    pub alpha_mode: AlphaMode,
}

impl Material for GlassMaterial {
    fn alpha_mode(&self) -> AlphaMode {
        self.alpha_mode
    }
    
    fn fragment_shader() -> ShaderRef {
        "shaders/glass.wgsl".into()
    }
}

// 创建不同透明度的材质
let opaque_material = materials.add(GlassMaterial {
    color: Color::WHITE,
    alpha_mode: AlphaMode::Opaque,
});

let transparent_material = materials.add(GlassMaterial {
    color: Color::rgba(0.8, 0.9, 1.0, 0.5),
    alpha_mode: AlphaMode::Blend,
});

let additive_material = materials.add(GlassMaterial {
    color: Color::rgb(0.2, 0.5, 1.0),
    alpha_mode: AlphaMode::Add,
});
```

---

### 3. MaterialProperties（材质属性）

**文件**: [`lib.rs`](file:///d:/work/ttc/bevy/crates/bevy_material/src/lib.rs)

#### MaterialProperties 结构

```rust
#[derive(Default)]
pub struct MaterialProperties {
    // 渲染方法
    pub render_method: OpaqueRendererMethod,
    pub alpha_mode: AlphaMode,
    
    // 管线键
    pub mesh_pipeline_key_bits: ErasedMeshPipelineKey,
    
    // 深度偏移
    pub depth_bias: f32,
    
    // 透射纹理
    pub reads_view_transmission_texture: bool,
    
    // 渲染阶段类型
    pub render_phase_type: RenderPhaseType,
    
    // 绑定组布局
    pub material_layout: Option<BindGroupLayoutDescriptor>,
    
    // 绘制函数（最多4个）
    pub draw_functions: SmallVec<[(InternedDrawFunctionLabel, DrawFunctionId); 4]>,
    
    // 着色器（最多3个）
    pub shaders: SmallVec<[(InternedShaderLabel, Handle<Shader>); 3]>,
    
    // Bindless 支持
    pub bindless: bool,
    
    // 专业化函数
    pub base_specialize: Option<BaseSpecializeFn>,
    pub prepass_specialize: Option<PrepassSpecializeFn>,
    pub user_specialize: Option<UserSpecializeFn>,
    
    // 材质键
    pub material_key: ErasedMaterialKey,
    
    // 功能开关
    pub shadows_enabled: bool,
    pub prepass_enabled: bool,
}
```

**MaterialProperties 方法**：

```rust
impl MaterialProperties {
    // 获取着色器
    pub fn get_shader(&self, label: impl ShaderLabel) -> Option<Handle<Shader>> {
        self.shaders
            .iter()
            .find(|(inner_label, _)| inner_label == &label.intern())
            .map(|(_, shader)| shader)
            .cloned()
    }
    
    // 添加着色器
    pub fn add_shader(&mut self, label: impl ShaderLabel, shader: Handle<Shader>) {
        self.shaders.push((label.intern(), shader));
    }
    
    // 获取绘制函数
    pub fn get_draw_function(&self, label: impl DrawFunctionLabel) -> Option<DrawFunctionId> {
        self.draw_functions
            .iter()
            .find(|(inner_label, _)| inner_label == &label.intern())
            .map(|(_, shader)| shader)
            .cloned()
    }
}
```

---

### 4. Pipeline Key（管线键）

**文件**: [`key.rs`](file:///d:/work/ttc/bevy/crates/bevy_material/src/key.rs)

#### 管线键类型

```rust
// 网格管线键（类型擦除）
#[derive(Clone, Copy)]
pub struct ErasedMeshPipelineKey {
    bits: u64,           // 管线标志位
    type_id: TypeId,     // 类型 ID（用于 downcast）
}

impl ErasedMeshPipelineKey {
    pub fn new<T: 'static>(key: T) -> Self
    where
        u64: From<T>,
    {
        Self {
            bits: key.into(),
            type_id: TypeId::of::<T>(),
        }
    }
    
    pub fn downcast<T: 'static + From<u64>>(&self) -> T {
        assert_eq!(self.type_id, TypeId::of::<T>());
        self.bits.into()
    }
}

// 材质管线键（组合网格和材质键）
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ErasedMaterialPipelineKey {
    pub mesh_key: ErasedMeshPipelineKey,
    pub material_key: ErasedMaterialKey,
    pub type_id: TypeId,
}

// 材质键（类型擦除）
pub struct ErasedMaterialKey {
    type_id: TypeId,
    hash: u64,
    value: Box<dyn Any + Send + Sync>,
    vtable: Arc<ErasedMaterialKeyVTable>,
}
```

**管线键用途**：

```rust
// 管线键用于缓存和查找渲染管线
pub struct PipelineCache {
    pipelines: HashMap<ErasedMaterialPipelineKey, CachedPipelineId>,
}

// 每个唯一的管线键对应一个唯一的渲染管线
let pipeline_key = ErasedMaterialPipelineKey {
    mesh_key: mesh_pipeline_key,
    material_key: material_key,
    type_id: TypeId::of::<MyMaterial>(),
};

let pipeline_id = pipeline_cache.get_or_create(pipeline_key, || {
    // 创建渲染管线
});
```

---

### 5. RenderPipelineDescriptor（渲染管线描述符）

**文件**: [`descriptor.rs`](file:///d:/work/ttc/bevy/crates/bevy_material/src/descriptor.rs)

#### RenderPipelineDescriptor 结构

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct RenderPipelineDescriptor {
    // 调试标签
    pub label: Option<Cow<'static, str>>,
    
    // 绑定组布局
    pub layout: Vec<BindGroupLayoutDescriptor>,
    
    // Push 常量范围
    pub push_constant_ranges: Vec<PushConstantRange>,
    
    // 顶点着色器状态
    pub vertex: VertexState,
    
    // 图元装配和光栅化
    pub primitive: PrimitiveState,
    
    // 深度/模板状态
    pub depth_stencil: Option<DepthStencilState>,
    
    // 多采样状态
    pub multisample: MultisampleState,
    
    // 片段着色器状态
    pub fragment: Option<FragmentState>,
    
    // 工作组内存初始化
    pub zero_initialize_workgroup_memory: bool,
}
```

#### VertexState（顶点状态）

```rust
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct VertexState {
    // 顶点着色器
    pub shader: Handle<Shader>,
    
    // 着色器定义（条件编译）
    pub shader_defs: Vec<ShaderDefVal>,
    
    // 入口点（默认 "vertex"）
    pub entry_point: Option<Cow<'static, str>>,
    
    // 顶点缓冲区布局
    pub buffers: Vec<VertexBufferLayout>,
}
```

#### FragmentState（片段状态）

```rust
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct FragmentState {
    // 片段着色器
    pub shader: Handle<Shader>,
    
    // 着色器定义
    pub shader_defs: Vec<ShaderDefVal>,
    
    // 入口点（默认 "fragment"）
    pub entry_point: Option<Cow<'static, str>>,
    
    // 颜色目标（渲染目标）
    pub targets: Vec<Option<ColorTargetState>>,
}
```

#### BindGroupLayoutDescriptor（绑定组布局描述符）

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct BindGroupLayoutDescriptor {
    // 调试标签
    pub label: Cow<'static, str>,
    
    // 绑定组条目
    pub entries: Vec<BindGroupLayoutEntry>,
}

impl BindGroupLayoutDescriptor {
    pub fn new(label: impl Into<Cow<'static, str>>, entries: &[BindGroupLayoutEntry]) -> Self {
        Self {
            label: label.into(),
            entries: entries.into(),
        }
    }
}
```

---

### 6. Pipeline Specialization（管线专业化）

**文件**: [`specialize.rs`](file:///d:/work/ttc/bevy/crates/bevy_material/src/specialize.rs)

#### 专业化函数类型

```rust
// 基础专业化函数（主渲染通道）
pub type BaseSpecializeFn = fn(
    &mut World,
    ErasedMaterialPipelineKey,
    &MeshVertexBufferLayoutRef,
    &Arc<MaterialProperties>,
) -> Result<CachedRenderPipelineId, SpecializedMeshPipelineError>;

// 预渲染通道专业化函数
pub type PrepassSpecializeFn = fn(
    &mut World,
    ErasedMaterialPipelineKey,
    &MeshVertexBufferLayoutRef,
    &Arc<MaterialProperties>,
) -> Result<CachedRenderPipelineId, SpecializedMeshPipelineError>;

// 用户自定义专业化函数
pub type UserSpecializeFn = fn(
    &dyn Any,
    &mut RenderPipelineDescriptor,
    &MeshVertexBufferLayoutRef,
    ErasedMaterialPipelineKey,
) -> Result<(), SpecializedMeshPipelineError>;
```

#### 专业化流程

```rust
// 1. 检查缓存
let pipeline_id = pipeline_cache.get(pipeline_key);

if pipeline_id.is_none() {
    // 2. 如果缓存未命中，创建新管线
    let mut descriptor = base_pipeline_descriptor.clone();
    
    // 3. 应用专业化
    if let Some(specialize_fn) = material_properties.base_specialize {
        let result = specialize_fn(
            &mut world,
            pipeline_key,
            &vertex_buffer_layout,
            &material_properties,
        );
        
        match result {
            Ok(id) => pipeline_id = Some(id),
            Err(e) => error!("Pipeline specialization failed: {}", e),
        }
    }
    
    // 4. 缓存管线
    pipeline_cache.insert(pipeline_key, pipeline_id.unwrap());
}
```

#### 专业化示例

```rust
// 基于 alpha 模式的专业化
fn specialize_pipeline(
    world: &mut World,
    key: ErasedMaterialPipelineKey,
    vertex_layout: &MeshVertexBufferLayoutRef,
    properties: &Arc<MaterialProperties>,
) -> Result<CachedRenderPipelineId, SpecializedMeshPipelineError> {
    let material_key = key.material_key.to_key::<MyMaterialKey>();
    
    // 创建基础管线描述符
    let mut descriptor = RenderPipelineDescriptor {
        vertex: VertexState {
            shader: asset_server.load("shaders/vertex.wgsl"),
            buffers: vec![vertex_layout.layout.clone()],
            ..default()
        },
        fragment: Some(FragmentState {
            shader: asset_server.load("shaders/fragment.wgsl"),
            targets: vec![Some(ColorTargetState {
                format: TextureFormat::Rgba16Float,
                blend: None,  // 将在下面设置
                write_mask: ColorWrites::ALL,
            })],
            ..default()
        }),
        ..default()
    };
    
    // 根据 alpha 模式设置混合
    match properties.alpha_mode {
        AlphaMode::Opaque => {
            // 不透明：无混合
        }
        AlphaMode::Blend => {
            descriptor.fragment_mut()?.targets[0] = Some(ColorTargetState {
                format: TextureFormat::Rgba16Float,
                blend: Some(BlendState {
                    color: BlendComponent {
                        src_factor: BlendFactor::SrcAlpha,
                        dst_factor: BlendFactor::OneMinusSrcAlpha,
                        operation: BlendOperation::Add,
                    },
                    alpha: BlendComponent {
                        src_factor: BlendFactor::One,
                        dst_factor: BlendFactor::OneMinusSrcAlpha,
                        operation: BlendOperation::Add,
                    },
                }),
                write_mask: ColorWrites::ALL,
            });
        }
        AlphaMode::Add => {
            // 加法混合
            // ...
        }
        // ... 其他模式
    }
    
    // 创建管线并缓存
    let pipeline = render_device.create_render_pipeline(&descriptor);
    let pipeline_id = pipeline_cache.add(pipeline);
    
    Ok(pipeline_id)
}
```

---

### 7. OpaqueRendererMethod（不透明渲染方法）

**文件**: [`opaque.rs`](file:///d:/work/ttc/bevy/crates/bevy_material/src/opaque.rs)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum OpaqueRendererMethod {
    // 前向渲染（每个物体单独计算光照）
    Forward,
    
    // 延迟渲染（先渲染 G-Buffer，再计算光照）
    Deferred,
    
    // 自动选择（基于全局设置）
    Auto,
}
```

**前向 vs 延迟**：

| 特性 | 前向渲染 | 延迟渲染 |
|------|----------|----------|
| **性能** | 少量光源快 | 大量光源快 |
| **内存** | 低 | 高（G-Buffer） |
| **MSAA** | 支持 | 不支持 |
| **复杂度** | 简单 | 复杂 |
| **适用场景** | 简单场景 | 复杂场景 |

---

### 8. Labels（标签系统）

**文件**: [`labels.rs`](file:///d:/work/ttc/bevy/crates/bevy_material/src/labels.rs)

#### 标签类型

```rust
// 绘制函数标签
pub trait DrawFunctionLabel: Clone + PartialEq + Eq + Hash + Send + Sync + 'static {
    fn intern(&self) -> InternedDrawFunctionLabel;
}

// 着色器标签
pub trait ShaderLabel: Clone + PartialEq + Eq + Hash + Send + Sync + 'static {
    fn intern(&self) -> InternedShaderLabel;
}

// 绘制函数 ID
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DrawFunctionId(pub u32);
```

**使用示例**：

```rust
// 定义自定义标签
#[derive(DrawFunctionLabel, Debug, Clone, PartialEq, Eq, Hash)]
enum MyDrawFunctions {
    Opaque,
    Transparent,
    Prepass,
}

// 在材质属性中注册
material_properties.add_draw_function(
    MyDrawFunctions::Opaque,
    draw_function_id,
);

// 查找绘制函数
let draw_function_id = material_properties.get_draw_function(MyDrawFunctions::Opaque);
```

---

## 典型使用示例

### 1. 简单自定义材质

```rust
use bevy::prelude::*;
use bevy_pbr::Material;
use bevy_render::render_resource::AsBindGroup;
use bevy_shader::ShaderRef;

// 定义材质
#[derive(AsBindGroup, Debug, Clone, Asset, TypePath)]
pub struct CustomMaterial {
    #[uniform(0)]
    pub color: Color,
    
    #[texture(1)]
    #[sampler(2)]
    pub texture: Option<Handle<Image>>,
    
    #[uniform(3)]
    pub roughness: f32,
}

// 实现 Material trait
impl Material for CustomMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/custom_material.wgsl".into()
    }
    
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }
}

// 注册材质插件
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(MaterialPlugin::<CustomMaterial>::default())
        .add_systems(Startup, setup)
        .run();
}

// 使用材质
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<CustomMaterial>>,
    asset_server: Res<AssetServer>,
) {
    // 创建材质实例
    let material = materials.add(CustomMaterial {
        color: Color::rgb(0.8, 0.6, 0.4),
        texture: Some(asset_server.load("textures/wood.png")),
        roughness: 0.8,
    });
    
    // 创建立方体
    commands.spawn(PbrBundle {
        mesh: meshes.add(Cuboid::default()),
        material: material,
        transform: Transform::from_xyz(0.0, 0.5, 0.0),
        ..default()
    });
    
    // 添加相机
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });
}
```

**对应的 WGSL 着色器** (`shaders/custom_material.wgsl`)：

```wgsl
// 顶点着色器（使用默认）

// 片段着色器
@group(3) @binding(0) var<uniform> color: vec4<f32>;
@group(3) @binding(1) var tex: texture_2d<f32>;
@group(3) @binding(2) var tex_sampler: sampler;
@group(3) @binding(3) var<uniform> roughness: f32;

@fragment
fn fragment(
    @location(0) uv: vec2<f32>,
) -> @location(0) vec4<f32> {
    // 采样纹理
    var sampled_color = textureSample(tex, tex_sampler, uv);
    
    // 应用颜色和粗糙度
    var final_color = sampled_color * color;
    final_color.rgb = mix(final_color.rgb, vec3(0.1), roughness);
    
    return final_color;
}
```

### 2. 高级材质（多个着色器）

```rust
#[derive(AsBindGroup, Debug, Clone, Asset, TypePath)]
pub struct AdvancedMaterial {
    #[uniform(0)]
    pub color: Color,
    
    // 多个纹理
    #[texture(1)]
    #[sampler(2)]
    pub albedo_texture: Option<Handle<Image>>,
    
    #[texture(3)]
    #[sampler(4)]
    pub normal_texture: Option<Handle<Image>>,
    
    #[texture(5)]
    #[sampler(6)]
    pub ao_texture: Option<Handle<Image>>,
}

impl Material for AdvancedMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/advanced_fragment.wgsl".into()
    }
    
    fn vertex_shader() -> ShaderRef {
        "shaders/advanced_vertex.wgsl".into()
    }
    
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}
```

### 3. 动态材质属性

```rust
#[derive(AsBindGroup, Debug, Clone, Asset, TypePath)]
pub struct AnimatedMaterial {
    #[uniform(0)]
    pub time: f32,
    
    #[uniform(1)]
    pub speed: f32,
    
    #[texture(2)]
    #[sampler(3)]
    pub texture: Handle<Image>,
}

impl Material for AnimatedMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/animated.wgsl".into()
    }
}

// 更新材质
fn update_material(
    mut materials: ResMut<Assets<AnimatedMaterial>>,
    time: Res<Time>,
) {
    for (_, material) in materials.iter_mut() {
        material.time = time.elapsed_seconds();
    }
}
```

---

## 材质系统工作流程

### 完整渲染流程

```
1. Extract Schedule (提取)
   ├─ 提取材质数据到渲染世界
   └─ 标记需要更新的材质

2. Render Schedule (渲染)
   ├─ Prepare Assets (准备资源)
   │  ├─ 加载纹理
   │  ├─ 创建 Uniform 缓冲区
   │  └─ 创建绑定组
   │
   ├─ Prepare Pipelines (准备管线)
   │  ├─ 计算 MaterialProperties
   │  ├─ 生成管线键
   │  └─ 专业化渲染管线
   │
   ├─ Queue (队列化)
   │  └─ 将网格加入渲染阶段
   │
   ├─ Phase Sort (阶段排序)
   │  └─ 按材质/深度排序
   │
   └─ Render (渲染)
      ├─ 设置绑定组
      ├─ 设置渲染管线
      └─ 执行绘制调用
```

### 关键系统

```rust
// 1. 提取系统
fn extract_materials(
    query: Extract<Query<&Handle<MyMaterial>>>),
    mut commands: Commands,
) {
    // 将材质句柄复制到渲染世界
}

// 2. 准备系统
fn prepare_materials(
    materials: Res<Assets<MyMaterial>>,
    render_assets: Res<RenderAssets<Image>>,
    mut render_materials: ResMut<RenderAssets<MyMaterial>>,
    render_device: Res<RenderDevice>,
) {
    for (asset_id, material) in materials.iter() {
        // 创建 GPU 资源
        let prepared_material = material.prepare(
            &render_device,
            &render_assets,
            &fallback_image,
        );
        
        render_materials.insert(asset_id, prepared_material);
    }
}

// 3. 队列系统
fn queue_materials(
    query: Query<(&Handle<Mesh>, &Handle<MyMaterial>, &GlobalTransform)>),
    mut render_phases: ResMut<RenderPhase<Opaque3d>>,
    render_materials: Res<RenderAssets<MyMaterial>>,
) {
    for (mesh_handle, material_handle, transform) in query.iter() {
        if let Some(prepared_material) = render_materials.get(material_handle) {
            render_phases.add(Opaque3d {
                entity: entity,
                pipeline: prepared_material.pipeline_id,
                draw_function: prepared_material.draw_function_id,
                // ...
            });
        }
    }
}
```

---

## 性能优化

### 1. 材质批处理

```rust
// 相同材质的物体自动批处理
commands.spawn_batch(vec![
    (PbrBundle {
        mesh: meshes.add(Cuboid::default()),
        material: my_material.clone(),
        transform: Transform::from_xyz(0.0, 0.0, 0.0),
        ..default()
    },),
    (PbrBundle {
        mesh: meshes.add(Cuboid::default()),
        material: my_material.clone(),  // 相同材质
        transform: Transform::from_xyz(1.0, 0.0, 0.0),
        ..default()
    },),
]);
```

### 2. 管线缓存

```rust
// Bevy 自动缓存渲染管线
// 相同管线键的物体重用同一管线
let pipeline_key = ErasedMaterialPipelineKey {
    mesh_key: mesh_key,
    material_key: material_key,
    type_id: TypeId::of::<MyMaterial>(),
};

// 缓存命中：重用现有管线
// 缓存未命中：创建新管线
```

### 3. 减少材质变体

```rust
// 避免：每个物体不同材质
for i in 0..1000 {
    materials.add(MyMaterial {
        color: Color::hsl(i as f32 / 1000.0 * 360.0, 1.0, 0.5),
        // ...
    });
}

// 更好：使用 instancing
#[derive(AsBindGroup, Debug, Clone, Asset, TypePath)]
pub struct InstancedMaterial {
    #[uniform(0)]
    pub base_color: Color,
}

// 每个实例有自己的颜色
#[derive(Component, ShaderType)]
pub struct InstanceColor {
    pub color: Color,
}
```

---

## 设计特点

### 1. 类型安全
- **编译时检查**：派生宏在编译时验证材质定义
- **类型转换**：自动实现 `ShaderType` trait
- **错误提示**：清晰的编译错误信息

### 2. 可扩展性
- **自定义材质**：轻松添加新材质类型
- **自定义着色器**：完全控制着色器代码
- **专业化**：基于材质属性的动态管线

### 3. 性能
- **批处理**：相同材质自动批处理
- **缓存**：管线和资源缓存
- **最小化状态切换**：按材质排序

### 4. 易用性
- **派生宏**：减少样板代码
- **默认实现**：`Material` trait 有合理默认
- **热重载**：支持运行时材质编辑

---

## 文件结构

```
src/
├── alpha.rs                    # AlphaMode 定义
├── key.rs                      # 管线键管理
├── descriptor.rs               # 渲染管线描述符
├── specialize.rs               # 管线专业化
├── labels.rs                   # 标签系统
├── bind_group_layout_entries.rs # 绑定组布局生成
├── opaque.rs                   # 不透明渲染方法
├── phase.rs                    # 渲染阶段类型
└── lib.rs                      # 主入口和 MaterialProperties
```

---

## 总结

`bevy_material` 是一个**强大且灵活的材质系统**，具有以下优势：

**核心优势**：
1. **类型安全**：编译时验证，运行时错误少
2. **高性能**：自动批处理和缓存
3. **可扩展**：支持复杂自定义材质
4. **易用**：派生宏减少样板代码
5. **灵活**：多种透明度模式和渲染方法

**适用场景**：
- 游戏开发（角色、道具、环境）
- 可视化（科学、数据）
- 编辑器工具
- 任何需要自定义渲染的应用

**学习资源**：
- [Bevy Material 文档](https://docs.rs/bevy/latest/bevy/pbr/trait.Material.html)
- [Bevy Examples](https://github.com/bevyengine/bevy/tree/main/examples)
- [WGSL 规范](https://www.w3.org/TR/WGSL/)

---

**注意**：`bevy_material` 是底层材质系统，大多数用户会使用 `bevy_pbr` 中的 `StandardMaterial` 或基于 `Material` trait 创建自定义材质。理解这个系统有助于创建高效和高质量的渲染效果。
