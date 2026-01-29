# bevy_pbr 模块总结

## 概述

`bevy_pbr` 是 Bevy 游戏引擎的**基于物理的渲染（PBR）模块**，提供了高质量的 3D 渲染能力。它实现了现代 PBR 标准，支持复杂材质、高级光照、阴影和各种视觉效果，是 Bevy 3D 渲染的核心组件。

**核心特性**：
- **PBR 材质系统**：基于物理的材质属性
- **高级光照**：点光源、方向光、聚光灯、环境光
- **阴影系统**：级联阴影、点光源阴影、接触阴影
- **屏幕空间效果**：SSAO（环境光遮蔽）、SSR（屏幕空间反射）
- **大气散射**：真实的天空和大气渲染
- **体积雾**：体积光和雾效果
- **贴花系统**：动态表面贴花
- **光照探针**：环境光照探针和辐照度体积

---

## 核心架构

### PBR 渲染流程

```
[Prepass] → [Main Pass] → [Post Processing]
   ↓            ↓                ↓
深度/法线   几何+光照    色调映射/SSR
  缓冲      计算        SSAO/雾
```

**关键阶段**：
1. **Prepass**：渲染深度、法线、运动向量
2. **Main Pass**：执行 PBR 光照计算
3. **Clustered Lighting**：分块光照（优化多光源）
4. **Post Processing**：屏幕空间效果、色调映射

---

## 主要子模块

### 模块功能总览

| 模块 | 功能 | 关键组件 |
|------|------|----------|
| **pbr_material** | PBR 标准材质 | `StandardMaterial`, `PbrMaterialPlugin` |
| **render** | 核心渲染逻辑 | `MeshPipeline`, `DrawMesh`, 光照系统 |
| **prepass** | 预渲染通道 | `PrepassPipelinePlugin`, 深度/法线缓冲 |
| **cluster** | 分块光照 | `ClusteredForward`, `GpuClusterableObject` |
| **ssao** | 屏幕空间环境光遮蔽 | `ScreenSpaceAmbientOcclusionPlugin` |
| **ssr** | 屏幕空间反射 | `ScreenSpaceReflectionsPlugin` |
| **atmosphere** | 大气散射 | `AtmospherePlugin`, 天空渲染 |
| **volumetric_fog** | 体积雾 | `VolumetricFogPlugin`, 体积光 |
| **light_probe** | 光照探针 | `EnvironmentMapLight`, `IrradianceVolume` |
| **lightmap** | 光照贴图 | `LightmapPlugin`, 烘焙光照 |
| **decal** | 贴花系统 | `Decal`, `ClusteredDecalPlugin` |
| **deferred** | 延迟渲染 | `DeferredPbrLightingPlugin`, G-Buffer |
| **contact_shadows** | 接触阴影 | `ContactShadowsPlugin` |
| **fog** | 距离雾 | `DistanceFog`, `FogFalloff` |
| **meshlet** | 网格分片（实验） | `MeshletPlugin`, 高效 GPU 渲染 |

---

## 核心子模块详解

### 1. PBR 材质系统

**文件**: [`pbr_material.rs`](file:///d:/work/ttc/bevy/crates/bevy_pbr/src/pbr_material.rs)

#### StandardMaterial 结构

```rust
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct StandardMaterial {
    // 基础颜色
    pub base_color: Color,
    pub base_color_texture: Option<Handle<Image>>,
    
    // 发光
    pub emissive: LinearRgba,
    pub emissive_texture: Option<Handle<Image>>,
    
    // PBR 属性
    pub perceptual_roughness: f32,      // 粗糙度 [0.089, 1.0]
    pub metallic: f32,                   // 金属度 [0.0, 1.0]
    pub reflectance: f32,                // 反射率
    
    // 纹理
    pub metallic_roughness_texture: Option<Handle<Image>>,
    pub normal_map_texture: Option<Handle<Image>>,
    pub occlusion_texture: Option<Handle<Image>>,
    
    // 透明度
    pub alpha_mode: AlphaMode,
    pub alpha_cutoff: f32,
    
    // 高级特性
    pub double_sided: bool,
    pub cull_mode: Option<Face>,
    pub depth_bias: f32,
}
```

#### 材质属性详解

| 属性 | 范围 | 说明 |
|------|------|------|
| **perceptual_roughness** | [0.089, 1.0] | 0=镜面光滑, 1=完全粗糙 |
| **metallic** | [0.0, 1.0] | 0=电介质(塑料), 1=金属 |
| **reflectance** | [0.0, 1.0] | 菲涅尔反射率 |
| **emissive** | [0, ∞) | 发光强度 (cd/m²) |
| **alpha_cutoff** | [0.0, 1.0] | Alpha 测试阈值 |

#### 材质纹理

```rust
// 标准纹理集
- base_color_texture: 基础颜色 (RGB)
- metallic_roughness_texture: 金属度(R) + 粗糙度(G)
- normal_map_texture: 法线贴图 (RGB)
- occlusion_texture: 环境光遮蔽 (R)
- emissive_texture: 发光贴图 (RGB)
- specular_map_texture: 高光贴图 (RGB)
- clearcoat_texture: 清漆层 (R)
- clearcoat_normal_texture: 清漆法线 (RGB)
- anisotropy_texture: 各向异性 (RG)
```

#### UV 通道

```rust
pub enum UvChannel {
    Uv0,  // 默认 UV 通道
    Uv1,  // 第二 UV 通道（用于光照贴图）
}

// 每个纹理都可以指定 UV 通道
pub struct StandardMaterial {
    pub base_color_channel: UvChannel,
    pub emissive_channel: UvChannel,
    // ...
}
```

---

### 2. 渲染系统

**文件**: [`render/mod.rs`](file:///d:/work/ttc/bevy/crates/bevy_pbr/src/render/mod.rs)

#### 核心渲染组件

```rust
// Mesh 管线
pub struct MeshPipeline;

// 绘制函数
pub struct DrawMesh;

// 管线键
pub struct MeshPipelineKey {
    // MSAA, 法线贴图, 透明度模式等
}

// 网格实例
pub struct RenderMeshInstances {
    pub instances: Vec<MeshInstance>,
}
```

#### 渲染流程

```rust
// 1. Extract: 提取网格数据
fn extract_meshes(
    query: Extract<Query<(&Handle<Mesh>, &GlobalTransform, &StandardMaterial)>>,
    mut commands: Commands,
) {
    // 将网格添加到渲染世界
}

// 2. Prepare: 准备 GPU 资源
fn prepare_meshes(
    render_meshes: Res<RenderAssets<Mesh>>,
    mut pipeline_cache: ResMut<PipelineCache>,
) {
    // 创建渲染管线和缓冲区
}

// 3. Queue: 加入渲染阶段
fn queue_meshes(
    query: Query<(&RenderMesh, &GlobalTransform, &PreparedMaterial<StandardMaterial>)>,
    mut render_phases: ResMut<RenderPhase<Opaque3d>>,
) {
    // 将网格加入不透明渲染阶段
}

// 4. Sort: 排序
fn sort_meshes(
    mut render_phases: ResMut<RenderPhase<Opaque3d>>,
) {
    // 按材质分桶，减少管线切换
}

// 5. Render: 执行渲染
fn render_meshes(
    mut render_pass: TrackedRenderPass,
    render_phases: Res<RenderPhase<Opaque3d>>,
) {
    // 执行 PBR 着色器
}
```

---

### 3. 光照系统

**文件**: [`render/light.rs`](file:///d:/work/ttc/bevy/crates/bevy_pbr/src/render/light.rs)

#### 支持的光源类型

```rust
// 方向光（太阳光）
pub struct ExtractedDirectionalLight {
    pub color: LinearRgba,
    pub illuminance: f32,      // 照度 (lux)
    pub transform: GlobalTransform,
    pub shadow_maps_enabled: bool,
    pub cascades: Option<Cascades>,  // 级联阴影
}

// 点光源
pub struct ExtractedPointLight {
    pub color: LinearRgba,
    pub intensity: f32,        // 光强 (lm/sr)
    pub range: f32,            // 影响范围
    pub radius: f32,           // 光源半径
    pub transform: GlobalTransform,
    pub shadow_maps_enabled: bool,
    pub volumetric: bool,      // 体积光
}

// 聚光灯
pub struct ExtractedSpotLight {
    pub color: LinearRgba,
    pub intensity: f32,
    pub range: f32,
    pub inner_angle: f32,      // 内圆锥角
    pub outer_angle: f32,      // 外圆锥角
    pub transform: GlobalTransform,
}

// 环境光
pub struct AmbientLight {
    pub color: Color,
    pub brightness: f32,
}
```

#### 分块光照 (Clustered Lighting)

**文件**: [`cluster.rs`](file:///d:/work/ttc/bevy/crates/bevy_pbr/src/cluster.rs)

**核心概念**：
```rust
// 将视锥体划分为 3D 网格（分块）
pub struct ExtractedClusterConfig {
    pub near: f32,
    pub far: f32,
    pub dimensions: UVec3,  // X, Y, Z 分块数量
}

// GPU 上的分块数据
#[derive(ShaderType)]
pub struct GpuClusterableObject {
    pub color_inverse_square_range: Vec4,
    pub position_radius: Vec4,
    pub flags: u32,
    // ...
}
```

**工作原理**：
```text
视锥体被划分为 N×M×L 个分块
每个分块存储影响它的光源列表
渲染时只计算影响当前像素的分块中的光源

优势：O(光源数) → O(分块数)，支持数千个光源
```

**限制**：
```rust
// 每个 UBO 最多 204 个光源
pub const MAX_UNIFORM_BUFFER_CLUSTERABLE_OBJECTS: usize = 204;

// 分块前向渲染需要 3 个存储缓冲区
pub const CLUSTERED_FORWARD_STORAGE_BUFFER_COUNT: u32 = 3;
```

---

### 4. 阴影系统

#### 阴影类型

```rust
// 方向光阴影：级联阴影贴图 (CSM)
pub struct DirectionalLightShadowMap {
    pub size: u32,           // 阴影贴图分辨率
    pub depth_bias: f32,     // 深度偏移
    pub normal_bias: f32,    // 法线偏移
    pub cascades: CascadeShadowConfig,
}

// 点光源阴影：立方体阴影贴图
pub struct PointLightShadowMap {
    pub size: u32,
    pub depth_bias: f32,
    pub normal_bias: f32,
}

// 接触阴影：软阴影效果
pub struct ContactShadows {
    pub max_distance: f32,   // 最大距离
    pub fade_start: f32,     // 淡出起始
    pub sample_count: u32,   // 采样数
    pub thickness: f32,      // 厚度
}
```

#### 阴影过滤

```rust
pub enum ShadowFilteringMethod {
    None,           // 无过滤（锯齿）
    PCF,            // 百分比渐近过滤
    PCFSoft,        // 软 PCF
    ESM,            // 指数阴影贴图
    VSM,            // 方差阴影贴图
}
```

---

### 5. 预渲染通道 (Prepass)

**文件**: [`prepass/mod.rs`](file:///d:/work/ttc/bevy/crates/bevy_pbr/src/prepass/mod.rs)

#### Prepass 类型

```rust
// 深度预渲染
pub struct DepthPrepass;

// 法线预渲染
pub struct NormalPrepass;

// 运动向量预渲染
pub struct MotionVectorPrepass;

// 延迟渲染预渲染
pub struct DeferredPrepass;
```

#### Prepass 输出

```rust
// 深度缓冲 (用于阴影、SSAO、SSR)
pub const DEPTH_FORMAT: TextureFormat = TextureFormat::Depth32Float;

// 法线缓冲 (用于光照计算、SSAO)
pub const NORMAL_PREPASS_FORMAT: TextureFormat = TextureFormat::Rgb10a2Unorm;

// 运动向量 (用于运动模糊)
pub const MOTION_VECTOR_PREPASS_FORMAT: TextureFormat = TextureFormat::Rg16Float;

// 延迟 G-Buffer
pub const DEFERRED_PREPASS_FORMAT: TextureFormat = TextureFormat::Rgba16Float;
```

#### 使用 Prepass

```rust
use bevy_core_pipeline::prepass::{DepthPrepass, NormalPrepass};

// 启用预渲染的相机
commands.spawn((
    Camera3dBundle {
        transform: Transform::from_xyz(0.0, 2.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    },
    DepthPrepass,
    NormalPrepass,
));
```

---

### 6. 屏幕空间效果

#### SSAO (屏幕空间环境光遮蔽)

**文件**: [`ssao/mod.rs`](file:///d:/work/ttc/bevy/crates/bevy_pbr/src/ssao/mod.rs)

```rust
pub struct ScreenSpaceAmbientOcclusionPlugin {
    pub quality: SsaoQuality,  // 质量级别
    pub radius: f32,           // 采样半径
    pub intensity: f32,        // 强度
    pub bias: f32,             // 深度偏移
}

pub enum SsaoQuality {
    Low,    // 低质量（性能优先）
    Medium, // 中等质量
    High,   // 高质量（质量优先）
}
```

**工作原理**：
```text
1. 从深度缓冲重建视图位置
2. 在每个像素周围随机采样
3. 计算被遮挡的程度
4. 应用模糊和降噪
```

#### SSR (屏幕空间反射)

**文件**: [`ssr/mod.rs`](file:///d:/work/ttc/bevy/crates/bevy_pbr/src/ssr/mod.rs)

```rust
pub struct ScreenSpaceReflectionsPlugin {
    pub max_steps: u32,        // 最大光线步进数
    pub max_distance: f32,     // 最大反射距离
    pub roughness_fade: f32,   // 粗糙度淡出
}
```

**工作原理**：
```text
1. 从像素发射反射光线
2. 在深度缓冲上步进
3. 找到交点并采样颜色
4. 根据粗糙度混合结果
```

---

### 7. 大气散射 (Atmosphere)

**文件**: [`atmosphere/mod.rs`](file:///d:/work/ttc/bevy/crates/bevy_pbr/src/atmosphere/mod.rs)

#### 大气参数

```rust
pub struct Atmosphere {
    pub sun_position: Vec3,           // 太阳位置
    pub sun_illuminance: f32,         // 太阳照度
    pub rayleigh_coefficient: Vec3,   // 瑞利散射系数
    pub mie_coefficient: Vec3,        // 米氏散射系数
    pub rayleigh_scale_height: f32,   // 瑞利标高
    pub mie_scale_height: f32,        // 米氏标高
    pub mie_directional_g: f32,       // 米氏各向异性
}
```

#### LUT 生成

```rust
// 透射率 LUT (Transmittance LUT)
// 存储从太空到地面的透射率
pub struct TransmittanceLUT;

// 天空视图 LUT (Sky View LUT)
// 存储天空颜色
pub struct SkyViewLUT;

// 多重散射 LUT (Multiscattering LUT)
// 存储多重散射贡献
pub struct MultiscatteringLUT;
```

**使用示例**：

```rust
use bevy_pbr::atmosphere::AtmospherePlugin;

App::new()
    .add_plugins(AtmospherePlugin {
        dynamic: true,  // 动态更新（实时时间）
        ..default()
    })
    .insert_resource(Atmosphere {
        sun_position: Vec3::new(0.0, 1.0, 0.0),
        ..default()
    });
```

---

### 8. 体积雾 (Volumetric Fog)

**文件**: [`volumetric_fog/mod.rs`](file:///d:/work/ttc/bevy/crates/bevy_pbr/src/volumetric_fog/mod.rs)

```rust
pub struct VolumetricFogPlugin;

pub struct VolumetricFog {
    pub scattering_coefficient: f32,  // 散射系数
    pub absorption_coefficient: f32,  // 吸收系数
    pub anisotropy: f32,              // 各向异性
    pub max_distance: f32,            // 最大距离
}
```

**体积光**：
```rust
// 点光源和聚光灯可以启用体积光
pub struct ExtractedPointLight {
    pub volumetric: bool,  // 启用体积光
}
```

---

### 9. 光照探针 (Light Probe)

**文件**: [`light_probe/mod.rs`](file:///d:/work/ttc/bevy/crates/bevy_pbr/src/light_probe/mod.rs)

#### 环境贴图光照

```rust
pub struct EnvironmentMapLight {
    pub environment_map: Handle<Image>,  // HDR 环境贴图
    pub brightness: f32,                 // 亮度
    pub background: EnvironmentMapBackground,  // 背景模式
}

pub enum EnvironmentMapBackground {
    None,           // 无背景
    Color(Color),   // 纯色背景
    EnvironmentMap, // 环境贴图背景
}
```

#### 辐照度体积

```rust
pub struct IrradianceVolume {
    pub size: UVec3,           // 体积分辨率
    pub min: Vec3,             // 最小边界
    pub max: Vec3,             // 最大边界
    pub data: Vec<Vec3>,       // 辐照度数据
}
```

**使用示例**：

```rust
commands.spawn((
    EnvironmentMapLight {
        environment_map: asset_server.load("environment.hdr"),
        brightness: 1000.0,
        background: EnvironmentMapBackground::EnvironmentMap,
    },
));
```

---

### 10. 贴花系统 (Decal)

**文件**: [`decal/mod.rs`](file:///d:/work/ttc/bevy/crates/bevy_pbr/src/decal/mod.rs)

```rust
#[derive(Component)]
pub struct Decal {
    pub size: Vec3,                    // 贴花尺寸
    pub diffuse: Handle<Image>,        // 漫反射贴图
    pub normal_map: Option<Handle<Image>>,  // 法线贴图
    pub emission: Option<Handle<Image>>,    // 发光贴图
    pub opacity: f32,                  // 不透明度
    pub depth_bias: f32,               // 深度偏移
}
```

**贴花渲染**：
```rust
// 前向贴花（直接渲染）
pub struct ForwardDecalPlugin;

// 分块贴花（优化大量贴花）
pub struct ClusteredDecalPlugin;
```

**使用示例**：

```rust
commands.spawn((
    DecalBundle {
        decal: Decal {
            size: Vec3::new(2.0, 0.1, 2.0),
            diffuse: asset_server.load("decal_diffuse.png"),
            normal_map: Some(asset_server.load("decal_normal.png")),
            opacity: 0.8,
        },
        transform: Transform::from_xyz(0.0, 0.05, 0.0),
        ..default()
    },
));
```

---

### 11. 延迟渲染 (Deferred Rendering)

**文件**: [`deferred/mod.rs`](file:///d:/work/ttc/bevy/crates/bevy_pbr/src/deferred/mod.rs)

```rust
pub struct DeferredPbrLightingPlugin;

// G-Buffer 格式
pub const DEFERRED_PREPASS_FORMAT: TextureFormat = TextureFormat::Rgba16Float;
pub const DEFERRED_LIGHTING_PASS_ID_FORMAT: TextureFormat = TextureFormat::R8Uint;
```

**延迟渲染流程**：
```text
1. Prepass: 渲染 G-Buffer (法线、颜色、粗糙度、金属度)
2. Lighting Pass: 计算光照
3. Composition: 组合结果
```

**优势**：
- 支持大量光源（每个像素单独计算）
- 更复杂的光照模型

**劣势**：
- 更高的内存带宽
- 不支持 MSAA
- 透明物体需要前向渲染

---

### 12. 光照贴图 (Lightmap)

**文件**: [`lightmap/mod.rs`](file:///d:/work/ttc/bevy/crates/bevy_pbr/src/lightmap/mod.rs)

```rust
pub struct LightmapPlugin;

#[derive(Component)]
pub struct Lightmap {
    pub texture: Handle<Image>,        // 光照贴图
    pub uv_channel: UvChannel,         // UV 通道
    pub intensity: f32,                // 强度
}
```

**使用示例**：

```rust
commands.spawn((
    PbrBundle {
        mesh: meshes.add(Cuboid::default()),
        material: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            ..default()
        }),
        lightmap: Lightmap {
            texture: asset_server.load("lightmap.ktx2"),
            uv_channel: UvChannel::Uv1,
            intensity: 1000.0,
        },
        ..default()
    },
));
```

---

### 13. 距离雾 (Fog)

**文件**: [`fog.rs`](file:///d:/work/ttc/bevy/crates/bevy_pbr/src/fog.rs)

```rust
#[derive(Resource, Clone)]
pub struct DistanceFog {
    pub color: Color,              // 雾颜色
    pub falloff: FogFalloff,       // 衰减类型
    pub density: f32,              // 密度
}

pub enum FogFalloff {
    Linear { start: f32, end: f32 },      // 线性衰减
    Exponential,                          // 指数衰减
    ExponentialSquared,                   // 指数平方衰减
}
```

**使用示例**：

```rust
app.insert_resource(DistanceFog {
    color: Color::rgb(0.8, 0.85, 0.9),
    falloff: FogFalloff::Exponential { density: 0.1 },
});
```

---

### 14. 网格分片 (Meshlet - 实验)

**文件**: [`meshlet/mod.rs`](file:///d:/work/ttc/bevy/crates/bevy_pbr/src/meshlet/mod.rs)

**功能**：
- 将网格划分为小片 (meshlets)
- GPU 视锥剔除和遮挡剔除
- 高效的大规模场景渲染

```rust
pub struct MeshletPlugin;

pub struct MeshletMesh {
    // 分片数据
}
```

**优势**：
- 支持数百万三角形的场景
- 每帧动态视锥剔除
- 减少 GPU 负载

---

## 典型使用示例

### 1. 基本 PBR 材质

```rust
use bevy::prelude::*;
use bevy_pbr::prelude::*;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    // 创建金属材质
    let metallic_material = materials.add(StandardMaterial {
        base_color: Color::rgb(0.9, 0.9, 0.95),
        metallic: 1.0,
        perceptual_roughness: 0.1,
        ..default()
    });
    
    // 创建带纹理的材质
    let textured_material = materials.add(StandardMaterial {
        base_color_texture: Some(asset_server.load("textures/wood_diffuse.png")),
        metallic_roughness_texture: Some(asset_server.load("textures/wood_roughness.png")),
        normal_map_texture: Some(asset_server.load("textures/wood_normal.png")),
        ..default()
    });
    
    // 创建立方体
    commands.spawn(PbrBundle {
        mesh: meshes.add(Cuboid::default()),
        material: metallic_material,
        transform: Transform::from_xyz(-1.0, 0.0, 0.0),
        ..default()
    });
    
    // 创建球体
    commands.spawn(PbrBundle {
        mesh: meshes.add(Sphere::new(0.5)),
        material: textured_material,
        transform: Transform::from_xyz(1.0, 0.0, 0.0),
        ..default()
    });
}
```

### 2. 高级光照设置

```rust
fn setup_lights(mut commands: Commands) {
    // 方向光（太阳光）
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            color: Color::WHITE,
            illuminance: 100000.0,
            shadows_enabled: true,
            shadow_projection: OrthographicProjection {
                left: -10.0,
                right: 10.0,
                bottom: -10.0,
                top: 10.0,
                near: -10.0,
                far: 10.0,
                ..default()
            },
            cascade_shadow_config: CascadeShadowConfig {
                first_cascade_far_bound: 4.0,
                maximum_distance: 20.0,
                ..default()
            },
            ..default()
        },
        transform: Transform::from_rotation(Quat::from_euler(
            EulerRot::ZYX, 0.0, 1.0, -std::f32::consts::FRAC_PI_4
        )),
        ..default()
    });
    
    // 点光源
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            color: Color::rgb(1.0, 0.8, 0.6),
            intensity: 10000.0,
            range: 10.0,
            shadows_enabled: true,
            volumetric: true,
            ..default()
        },
        transform: Transform::from_xyz(0.0, 2.0, 0.0),
        ..default()
    });
    
    // 环境光
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 200.0,
    });
}
```

### 3. 屏幕空间效果

```rust
use bevy_pbr::ssao::ScreenSpaceAmbientOcclusionPlugin;
use bevy_pbr::ssr::ScreenSpaceReflectionsPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(ScreenSpaceAmbientOcclusionPlugin {
            quality: SsaoQuality::High,
            radius: 0.5,
            intensity: 1.0,
            bias: 0.025,
        })
        .add_plugins(ScreenSpaceReflectionsPlugin {
            max_steps: 200,
            max_distance: 50.0,
            roughness_fade: 0.2,
        })
        .run();
}
```

### 4. 大气和天空

```rust
use bevy_pbr::atmosphere::AtmospherePlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(AtmospherePlugin {
            dynamic: true,
            ..default()
        })
        .add_systems(Startup, setup_environment)
        .run();
}

fn setup_environment(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    // 环境贴图光照
    commands.spawn(EnvironmentMapLight {
        environment_map: asset_server.load("textures/environment.hdr"),
        brightness: 1000.0,
        background: EnvironmentMapBackground::EnvironmentMap,
    });
}
```

---

## 关键技术特性

### 1. 物理光照单位

```rust
// 照度 (lux) - 方向光
pub struct DirectionalLight {
    pub illuminance: f32,  // 单位: lux
}

// 光强 (lm/sr) - 点光源/聚光灯
pub struct PointLight {
    pub intensity: f32,    // 单位: lm/sr
}

// 发光强度 (cd/m²) - 材质
pub struct StandardMaterial {
    pub emissive: LinearRgba,  // 单位: cd/m² (nits)
}
```

**真实世界参考**：
- 月光: 0.1 lux
- 室内: 100-1000 lux
- 阴天: 10000 lux
- 晴天: 100000 lux

### 2. HDR 渲染

```rust
// 使用 HDR 格式
pub const HDR_FORMAT: TextureFormat = TextureFormat::Rgba16Float;

// 色调映射
pub enum Tonemapping {
    None,
    Aces,           // Academy Color Encoding System
    AgX,
    Filmic,
    Reinhard,
    // ...
}
```

### 3. 性能优化

```rust
// 分块光照 (支持数千光源)
pub const MAX_LIGHTS_PER_CLUSTER: usize = 204;

// GPU 实例化批处理
pub struct GpuPreprocessingMode;

// 视锥剔除
pub struct ViewVisibility;

// 遮挡剔除 (实验)
pub struct OcclusionCulling;
```

---

## 设计特点

### 1. 材质系统
- **基于 trait**：`Material` trait 支持自定义材质
- **派生宏**：`#[derive(AsBindGroup)]` 自动生成绑定组
- **热重载**：支持运行时材质编辑

### 2. 渲染管线
- **多通道**：前向、延迟、分块前向渲染
- **可扩展**：易于添加新的渲染通道
- **平台适配**：自动适配 WebGL2/WebGPU 能力

### 3. 光照系统
- **物理准确**：基于真实物理单位
- **可扩展**：支持自定义光源类型
- **性能优化**：分块光照、实例化

---

## 文件结构

```
src/
├── pbr_material.rs          # PBR 标准材质
├── material.rs              # 材质系统基础
├── mesh_material.rs         # 网格材质绑定
├── components.rs            # 核心组件
├── render/                  # 渲染逻辑
│   ├── mesh.rs              # 网格渲染
│   ├── light.rs             # 光照系统
│   ├── fog.rs               # 雾效果
│   ├── skin.rs              # 蒙皮动画
│   └── morph.rs             # 形态目标动画
├── prepass/                 # 预渲染通道
├── cluster.rs               # 分块光照
├── ssao/                    # 屏幕空间环境光遮蔽
├── ssr/                     # 屏幕空间反射
├── atmosphere/              # 大气散射
├── volumetric_fog/          # 体积雾
├── light_probe/             # 光照探针
├── lightmap/                # 光照贴图
├── decal/                   # 贴花系统
├── deferred/                # 延迟渲染
├── contact_shadows.rs       # 接触阴影
├── fog.rs                   # 距离雾
├── parallax.rs              # 视差映射
├── wireframe.rs             # 线框渲染
├── meshlet/                 # 网格分片（实验）
└── lib.rs                   # 主入口
```

---

## 总结

`bevy_pbr` 是一个**功能完整、高性能的 PBR 渲染系统**，具有以下优势：

**核心优势**：
1. **物理准确**：基于真实物理的光照和材质
2. **功能丰富**：支持高级特性（SSAO、SSR、体积雾等）
3. **高性能**：分块光照、GPU 实例化、视锥剔除
4. **可扩展**：易于添加自定义材质和渲染通道
5. **生产就绪**：用于实际游戏和应用

**适用场景**：
- 3D 游戏开发
- 建筑可视化
- 电影渲染
- 交互式 3D 应用

**学习资源**：
- [Filament Material Properties](https://google.github.io/filament/notes/material_properties.html)
- [Real-time Rendering Book](https://www.realtimerendering.com/)
- [Bevy Examples](https://github.com/bevyengine/bevy/tree/main/examples)

---

**注意**：`bevy_pbr` 是高级渲染模块，建立在 `bevy_render` 和 `bevy_core_pipeline` 之上。理解这些底层模块有助于充分利用 PBR 系统的能力。
