# bevy_light 模块总结

## 概述

`bevy_light` 是 Bevy 游戏引擎的**光照系统核心模块**，提供了完整的 3D 光照解决方案。它实现了多种光源类型、高级阴影技术、分块光照（Clustered Lighting）和环境光照，是 Bevy 渲染系统的重要组成部分。

**核心特性**：
- **多种光源类型**：方向光、点光源、聚光灯、环境光
- **高级阴影**：级联阴影贴图（CSM）、立方体阴影贴图、接触阴影
- **分块光照**：支持数千个动态光源
- **物理光照单位**：基于真实物理单位（lux、lumens）
- **环境光照**：环境贴图、光照探针、辐照度体积
- **体积光**：体积雾和体积光效果

---

## 核心架构

### 光照系统流程

```
[Light Definition] → [Frustum Culling] → [Clustering] → [Extraction] → [Rendering]
       ↓                  ↓                ↓              ↓               ↓
  定义光源组件      视锥剔除        分块光照      提取到渲染世界    GPU光照计算
```

**关键阶段**：
1. **定义**：创建光源实体（方向光、点光源、聚光灯）
2. **视锥剔除**：计算光源影响的视锥体
3. **分块**：将光源分配到视锥体分块（Clusters）
4. **提取**：提取光源数据到渲染世界
5. **渲染**：在着色器中计算光照贡献

---

## 主要子模块

### 模块功能总览

| 模块 | 功能 | 关键组件 |
|------|------|----------|
| **directional_light** | 方向光（太阳光） | `DirectionalLight`, `CascadeShadowConfig` |
| **point_light** | 点光源 | `PointLight`, `PointLightShadowMap` |
| **spot_light** | 聚光灯 | `SpotLight`, 锥形光 |
| **ambient_light** | 环境光 | `AmbientLight`, `GlobalAmbientLight` |
| **cascade** | 级联阴影 | `Cascades`, `CascadeShadowConfigBuilder` |
| **cluster** | 分块光照 | `Clusters`, `ClusterConfig`, `VisibleClusterableObjects` |
| **probe** | 光照探针 | `EnvironmentMapLight`, `IrradianceVolume`, `LightProbe` |
| **volumetric** | 体积光 | `VolumetricLight`, `VolumetricFog`, `FogVolume` |

---

## 核心子模块详解

### 1. DirectionalLight（方向光）

**文件**: [`directional_light.rs`](file:///d:/work/ttc/bevy/crates/bevy_light/src/directional_light.rs)

#### DirectionalLight 结构

```rust
#[derive(Component, Debug, Clone, Copy, Reflect)]
pub struct DirectionalLight {
    // 光源颜色
    pub color: Color,
    
    // 照度（单位：lux）
    pub illuminance: f32,
    
    // 阴影开关
    pub shadow_maps_enabled: bool,
    
    // 接触阴影
    pub contact_shadows_enabled: bool,
    
    // 软阴影（实验）
    #[cfg(feature = "experimental_pbr_pcss")]
    pub soft_shadow_size: Option<f32>,
    
    // 光照贴图影响
    pub affects_lightmapped_mesh_diffuse: bool,
    
    // 阴影偏移（避免阴影瑕疵）
    pub shadow_depth_bias: f32,
    pub shadow_normal_bias: f32,
}

impl Default for DirectionalLight {
    fn default() -> Self {
        DirectionalLight {
            color: Color::WHITE,
            illuminance: light_consts::lux::AMBIENT_DAYLIGHT,  // 10,000 lux
            shadow_maps_enabled: false,
            contact_shadows_enabled: false,
            shadow_depth_bias: 0.02,
            shadow_normal_bias: 1.8,
            affects_lightmapped_mesh_diffuse: true,
        }
    }
}
```

**照度参考值**（lux）：

| 场景 | 照度（lux） | 典型用途 |
|------|------------|----------|
| 无月夜晚 | 0.0001 | 星空光照 |
| 满月夜晚 | 0.05 | 月光 |
| 客厅 | 50 | 室内环境光 |
| 办公室 | 320-500 | 工作环境 |
| 阴天 | 1000 | 室外阴天 |
| 白天（非直射） | 10,000-25,000 | 环境日光 |
| 直射阳光 | 32,000-100,000 | 强烈阳光 |

#### 级联阴影配置

```rust
#[derive(Component, Debug, Clone, Reflect)]
pub struct CascadeShadowConfig {
    // 第一级联的远裁剪面
    pub first_cascade_far_bound: f32,
    
    // 最大阴影距离
    pub maximum_distance: f32,
    
    // 级联数量（通常 2-4）
    pub cascade_count: usize,
    
    // 级联分布（线性或对数）
    pub distribution: CascadeDistribution,
}

// 级联分布策略
enum CascadeDistribution {
    Linear,      // 均匀分布
    Geometric,   // 几何分布（近密远疏）
}
```

#### 方向光纹理

```rust
#[derive(Clone, Component, Debug, Reflect)]
pub struct DirectionalLightTexture {
    // 纹理图像（仅读取 R 通道）
    pub image: Handle<Image>,
    
    // 是否平铺
    pub tiled: bool,
}
```

**用途**：模拟窗户阴影、gobo 效果、光域网等

#### 阴影贴图资源

```rust
#[derive(Resource, Clone, Debug, Reflect)]
pub struct DirectionalLightShadowMap {
    // 每个级联的分辨率
    // 必须是 2 的幂次方
    pub size: usize,  // 默认 2048
}
```

**使用示例**：

```rust
use bevy::prelude::*;
use bevy_light::DirectionalLight;

fn setup_directional_light(mut commands: Commands) {
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            color: Color::rgb(1.0, 0.95, 0.9),
            illuminance: light_consts::lux::DIRECT_SUNLIGHT,  // 100,000 lux
            shadow_maps_enabled: true,
            ..default()
        },
        transform: Transform::from_rotation(Quat::from_euler(
            EulerRot::ZYX, 0.0, 1.0, -std::f32::consts::FRAC_PI_4
        )),
        ..default()
    });
    
    // 设置阴影贴图分辨率
    commands.insert_resource(DirectionalLightShadowMap { size: 4096 });
}
```

---

### 2. PointLight（点光源）

**文件**: [`point_light.rs`](file:///d:/work/ttc/bevy/crates/bevy_light/src/point_light.rs)

#### PointLight 结构

```rust
#[derive(Component, Debug, Clone, Copy, Reflect)]
pub struct PointLight {
    // 光源颜色
    pub color: Color,
    
    // 光强（单位：lumens）
    pub intensity: f32,
    
    // 影响范围
    pub range: f32,
    
    // 光源半径（模拟面光源）
    pub radius: f32,
    
    // 阴影开关
    pub shadow_maps_enabled: bool,
    
    // 接触阴影
    pub contact_shadows_enabled: bool,
    
    // 软阴影（实验）
    #[cfg(feature = "experimental_pbr_pcss")]
    pub soft_shadows_enabled: bool,
    
    // 光照贴图影响
    pub affects_lightmapped_mesh_diffuse: bool,
    
    // 阴影偏移
    pub shadow_depth_bias: f32,
    pub shadow_normal_bias: f32,
    
    // 阴影贴图近裁剪面
    pub shadow_map_near_z: f32,
}

impl Default for PointLight {
    fn default() -> Self {
        PointLight {
            color: Color::WHITE,
            intensity: light_consts::lumens::VERY_LARGE_CINEMA_LIGHT,  // 1,000,000 lm
            range: 20.0,
            radius: 0.0,
            shadow_maps_enabled: false,
            contact_shadows_enabled: false,
            affects_lightmapped_mesh_diffuse: true,
            shadow_depth_bias: 0.08,
            shadow_normal_bias: 0.6,
            shadow_map_near_z: 0.1,
        }
    }
}
```

**光强参考值**（lumens）：

| 光源类型 | 功率（W） | 光强（lm） |
|----------|-----------|-----------|
| LED 灯泡 | 3 | 200 |
| 40W 白炽灯 | 40 | 450 |
| 60W 白炽灯 | 60 | 800 |
| 100W 白炽灯 | 100 | 1600 |
| 影院灯 | - | 1,000,000 |

#### 点光源纹理

```rust
#[derive(Clone, Component, Debug, Reflect)]
pub struct PointLightTexture {
    // 立方体贴图
    pub image: Handle<Image>,
    
    // 立方体贴图布局
    pub cubemap_layout: CubemapLayout,
}
```

**用途**：模拟不同形状的光源（如手电筒、灯笼）

#### 阴影贴图资源

```rust
#[derive(Resource, Clone, Debug, Reflect)]
pub struct PointLightShadowMap {
    // 立方体每个面的分辨率
    pub size: usize,  // 默认 1024
}
```

**使用示例**：

```rust
use bevy::prelude::*;
use bevy_light::PointLight;

fn setup_point_light(mut commands: Commands) {
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            color: Color::rgb(1.0, 0.8, 0.6),
            intensity: 10000.0,  // 10,000 lumens
            range: 10.0,
            radius: 0.1,  // 小面光源
            shadow_maps_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(0.0, 2.0, 0.0),
        ..default()
    });
    
    // 设置点光源阴影贴图分辨率
    commands.insert_resource(PointLightShadowMap { size: 2048 });
}
```

---

### 3. SpotLight（聚光灯）

**文件**: [`spot_light.rs`](file:///d:/work/ttc/bevy/crates/bevy_light/src/spot_light.rs)

#### SpotLight 结构

```rust
#[derive(Component, Debug, Clone, Copy, Reflect)]
pub struct SpotLight {
    // 光源颜色
    pub color: Color,
    
    // 光强（单位：lumens）
    pub intensity: f32,
    
    // 影响范围
    pub range: f32,
    
    // 光源半径
    pub radius: f32,
    
    // 阴影开关
    pub shadow_maps_enabled: bool,
    
    // 接触阴影
    pub contact_shadows_enabled: bool,
    
    // 软阴影（实验）
    #[cfg(feature = "experimental_pbr_pcss")]
    pub soft_shadows_enabled: bool,
    
    // 光照贴图影响
    pub affects_lightmapped_mesh_diffuse: bool,
    
    // 阴影偏移
    pub shadow_depth_bias: f32,
    pub shadow_normal_bias: f32,
    
    // 阴影贴图近裁剪面
    pub shadow_map_near_z: f32,
    
    // 外圆锥角（光的有效范围）
    pub outer_angle: f32,  // 应 < PI/2.0
    
    // 内圆锥角（光的完全强度范围）
    pub inner_angle: f32,  // 应 <= outer_angle
}

impl Default for SpotLight {
    fn default() -> Self {
        SpotLight {
            color: Color::WHITE,
            intensity: 1_000_000.0,
            range: 20.0,
            radius: 0.0,
            shadow_maps_enabled: false,
            contact_shadows_enabled: false,
            affects_lightmapped_mesh_diffuse: true,
            shadow_depth_bias: 0.02,
            shadow_normal_bias: 1.8,
            shadow_map_near_z: 0.1,
            outer_angle: std::f32::consts::FRAC_PI_4,  // 45度
            inner_angle: std::f32::consts::FRAC_PI_8,  // 22.5度
        }
    }
}
```

**聚光灯角度说明**：

```text
         ┌─────────────────┐
         │                 │
         │   inner_angle   │  ← 完全强度区域
         │                 │
         ├─────────────────┤
         │                 │
         │   outer_angle   │  ← 衰减区域
         │                 │
         └─────────────────┘
```

**角度建议**：
- `outer_angle` 应小于 π/2.0（90度）
- 接近 π/2.0 时阴影会变得非常块状
- `inner_angle` 与 `outer_angle` 的差控制衰减速度

#### 使用示例

```rust
use bevy::prelude::*;
use bevy_light::SpotLight;

fn setup_spotlight(mut commands: Commands) {
    commands.spawn(SpotLightBundle {
        spot_light: SpotLight {
            color: Color::rgb(1.0, 0.95, 0.8),
            intensity: 50000.0,
            range: 15.0,
            outer_angle: std::f32::consts::FRAC_PI_6,  // 30度
            inner_angle: std::f32::consts::FRAC_PI_12,  // 15度
            shadow_maps_enabled: true,
            ..default()
        },
        transform: Transform {
            translation: Vec3::new(5.0, 5.0, 5.0),
            rotation: Quat::from_rotation_x(-0.5),
            ..default()
        },
        ..default()
    });
}
```

---

### 4. AmbientLight（环境光）

**文件**: [`ambient_light.rs`](file:///d:/work/ttc/bevy/crates/bevy_light/src/ambient_light.rs)

#### AmbientLight 结构

```rust
#[derive(Component, Debug, Clone, Copy, Reflect)]
pub struct AmbientLight {
    // 环境光颜色
    pub color: Color,
    
    // 环境光亮度
    pub brightness: f32,
}

impl Default for AmbientLight {
    fn default() -> Self {
        AmbientLight {
            color: Color::WHITE,
            brightness: 1.0,
        }
    }
}
```

#### GlobalAmbientLight 资源

```rust
#[derive(Resource, Default, Clone, Debug, Reflect)]
pub struct GlobalAmbientLight {
    // 全局环境光颜色
    pub color: Color,
    
    // 全局环境光亮度
    pub brightness: f32,
}
```

**使用示例**：

```rust
use bevy::prelude::*;
use bevy_light::AmbientLight;

fn setup_ambient_light(mut commands: Commands) {
    // 实体环境光
    commands.spawn(AmbientLight {
        color: Color::rgb(0.3, 0.4, 0.5),
        brightness: 0.5,
    });
    
    // 全局环境光
    commands.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: 200.0,  // 200 lux
    });
}
```

---

### 5. Clustered Lighting（分块光照）

**文件**: [`cluster/mod.rs`](file:///d:/work/ttc/bevy/crates/bevy_light/src/cluster/mod.rs)

#### 核心概念

```text
视锥体被划分为 N×M×L 个分块（Clusters）
每个分块存储影响它的光源列表
渲染时只计算影响当前像素的分块中的光源

优势：O(光源数) → O(分块数)，支持数千个光源
```

#### ClusterConfig 结构

```rust
#[derive(Debug, Copy, Clone, Component, Reflect)]
pub enum ClusterConfig {
    // 禁用分块计算
    None,
    
    // 单个分块（适合光源少的场景）
    Single,
    
    // 显式 XYZ 分块数量
    XYZ {
        dimensions: UVec3,  // X, Y, Z 分块数
        z_config: ClusterZConfig,
        dynamic_resizing: bool,  // 动态调整大小
    },
    
    // 固定 Z 切片数，自动计算 X/Y
    FixedZ {
        total: u32,        // 总分块数
        z_slices: u32,     // Z 切片数
        z_config: ClusterZConfig,
        dynamic_resizing: bool,
    },
}

// Z 轴配置
#[derive(Debug, Copy, Clone, Reflect)]
pub struct ClusterZConfig {
    // 第一个深度切片的远裁剪面
    pub first_slice_depth: f32,
    
    // 最远深度切片的计算方式
    pub far_z_mode: ClusterFarZMode,
}

// 远裁剪面模式
#[derive(Debug, Copy, Clone, Reflect)]
pub enum ClusterFarZMode {
    // 基于可见光源计算
    MaxClusterableObjectRange,
    
    // 固定值
    Constant(f32),
}
```

#### Clusters 组件

```rust
#[derive(Component, Debug, Default)]
pub struct Clusters {
    // 分块大小
    pub tile_size: UVec2,
    
    // X/Y/Z 分块数量
    pub dimensions: UVec3,
    
    // 近/远裁剪面
    pub near: f32,
    pub far: f32,
    
    // 每个分块的可聚簇对象
    pub clusterable_objects: Vec<VisibleClusterableObjects>,
}
```

#### VisibleClusterableObjects 结构

```rust
#[derive(Clone, Component, Debug, Default)]
pub struct VisibleClusterableObjects {
    // 实体列表
    pub entities: Vec<Entity>,
    
    // 对象计数
    pub counts: ClusterableObjectCounts,
}

// 对象计数
#[derive(Clone, Copy, Default, Debug)]
pub struct ClusterableObjectCounts {
    pub point_lights: u32,
    pub spot_lights: u32,
    pub reflection_probes: u32,
    pub irradiance_volumes: u32,
    pub decals: u32,
}
```

#### 使用示例

```rust
use bevy::prelude::*;
use bevy_light::cluster::ClusterConfig;

fn setup_cluster_config(mut commands: Commands) {
    // 为相机设置分块配置
    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(0.0, 2.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        },
        ClusterConfig::XYZ {
            dimensions: UVec3::new(16, 16, 24),  // 16×16×24 个分块
            z_config: ClusterZConfig {
                first_slice_depth: 0.1,
                far_z_mode: ClusterFarZMode::Constant(100.0),
            },
            dynamic_resizing: true,
        },
    ));
}
```

#### 分块光照工作原理

```rust
// 1. 计算分块
fn calculate_clusters(
    camera: &Camera,
    config: &ClusterConfig,
) -> Clusters {
    // 将视锥体划分为分块
    let clusters = Clusters::new(camera, config);
    clusters
}

// 2. 分配光源到分块
fn assign_lights_to_clusters(
    clusters: &mut Clusters,
    lights: &Query<&PointLight>,
) {
    for (entity, light) in lights.iter() {
        let affected_clusters = find_clusters_affected_by_light(&clusters, light);
        
        for cluster_id in affected_clusters {
            clusters.add_light(cluster_id, entity);
        }
    }
}

// 3. GPU 光照计算
// 在着色器中：
// - 计算当前像素所在的分块
// - 获取该分块中的光源列表
// - 只计算这些光源的贡献
```

---

### 6. Light Probe（光照探针）

**文件**: [`probe.rs`](file:///d:/work/ttc/bevy/crates/bevy_light/src/probe.rs)

#### EnvironmentMapLight 结构

```rust
#[derive(Component, Debug, Clone, Reflect)]
pub struct EnvironmentMapLight {
    // 环境贴图（HDR）
    pub environment_map: Handle<Image>,
    
    // 亮度
    pub brightness: f32,
    
    // 背景模式
    pub background: EnvironmentMapBackground,
}

// 背景模式
enum EnvironmentMapBackground {
    None,           // 无背景
    Color(Color),   // 纯色背景
    EnvironmentMap, // 环境贴图背景
}
```

#### GeneratedEnvironmentMapLight 结构

```rust
#[derive(Component, Debug, Clone, Reflect)]
pub struct GeneratedEnvironmentMapLight {
    // 天空生成配置
    pub sky_config: SkyConfig,
    
    // 亮度
    pub brightness: f32,
    
    // 背景模式
    pub background: EnvironmentMapBackground,
}
```

#### IrradianceVolume 结构

```rust
#[derive(Component, Debug, Clone, Reflect)]
pub struct IrradianceVolume {
    // 体积分辨率
    pub size: UVec3,
    
    // 边界
    pub min: Vec3,
    pub max: Vec3,
    
    // 辐照度数据
    pub data: Vec<Vec3>,
}
```

#### LightProbe 结构

```rust
#[derive(Component, Debug, Clone, Reflect)]
pub struct LightProbe {
    // 探针位置
    pub position: Vec3,
    
    // 探针半径
    pub radius: f32,
    
    // 辐照度数据（SH 系数）
    pub sh_coefficients: [Vec3; 9],
}
```

**使用示例**：

```rust
use bevy::prelude::*;
use bevy_light::EnvironmentMapLight;

fn setup_environment_light(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    commands.spawn(EnvironmentMapLight {
        environment_map: asset_server.load("environment.hdr"),
        brightness: 1000.0,
        background: EnvironmentMapBackground::EnvironmentMap,
    });
}
```

---

### 7. Volumetric Light（体积光）

**文件**: [`volumetric.rs`](file:///d:/work/ttc/bevy/crates/bevy_light/src/volumetric.rs)

#### VolumetricLight 结构

```rust
#[derive(Component, Debug, Clone, Copy, Reflect)]
pub struct VolumetricLight {
    // 散射系数
    pub scattering_coefficient: f32,
    
    // 吸收系数
    pub absorption_coefficient: f32,
    
    // 各向异性
    pub anisotropy: f32,
}
```

#### VolumetricFog 结构

```rust
#[derive(Component, Debug, Clone, Copy, Reflect)]
pub struct VolumetricFog {
    // 雾密度
    pub density: f32,
    
    // 雾颜色
    pub color: Color,
    
    // 雾高度
    pub height: f32,
    
    // 雾高度衰减
    pub height_falloff: f32,
}
```

#### FogVolume 结构

```rust
#[derive(Component, Debug, Clone, Reflect)]
pub struct FogVolume {
    // 体积形状
    pub shape: FogVolumeShape,
    
    // 雾密度
    pub density: f32,
    
    // 雾颜色
    pub color: Color,
}

// 体积形状
enum FogVolumeShape {
    Box { size: Vec3 },
    Sphere { radius: f32 },
    Cylinder { radius: f32, height: f32 },
}
```

**使用示例**：

```rust
use bevy::prelude::*;
use bevy_light::{PointLight, VolumetricLight};

fn setup_volumetric_light(mut commands: Commands) {
    commands.spawn((
        PointLightBundle {
            point_light: PointLight {
                color: Color::rgb(1.0, 0.9, 0.8),
                intensity: 10000.0,
                range: 10.0,
                ..default()
            },
            transform: Transform::from_xyz(0.0, 2.0, 0.0),
            ..default()
        },
        VolumetricLight {
            scattering_coefficient: 0.1,
            absorption_coefficient: 0.01,
            anisotropy: 0.9,
        },
    ));
}
```

---

## 物理光照单位

### 单位转换

```rust
// Lux（照度）= Lumens（光强） / Area（面积）
// 1 lux = 1 lm/m²

// 方向光使用 lux（因为是平行光，照度均匀）
// 点光源和聚光灯使用 lumens（因为是从点发射）

// 示例：
// 100W 白炽灯 ≈ 1600 lumens
// 如果在 1 米距离，照度 ≈ 1600 / (4 * π * 1²) ≈ 127 lux
```

### 真实世界参考

| 场景 | 照度（lux） | 典型光源 |
|------|------------|----------|
| 星光 | 0.0001 | 无月夜晚 |
| 月光 | 0.05 | 满月 |
| 室内 | 50-500 | 灯泡 |
| 阴天 | 1000 | 室外阴天 |
| 白天 | 10,000-25,000 | 非直射阳光 |
| 阳光 | 32,000-100,000 | 直射阳光 |

---

## 阴影技术

### 1. Cascaded Shadow Maps（CSM）

```text
视锥体被划分为多个级联（Cascades）
每个级联有自己的阴影贴图
近区域阴影贴图分辨率高，远区域分辨率低

优势：近处阴影清晰，远处阴影节省内存
```

### 2. Cube Shadow Maps（立方体阴影贴图）

```text
点光源使用 6 个方向的阴影贴图
形成立方体包围盒
每个方向一个阴影贴图

优势：支持全方向阴影
```

### 3. Contact Shadows（接触阴影）

```text
在物体接触处添加软阴影
使用光线步进（Ray Marching）

优势：增强真实感，特别是小物体
```

### 4. Percentage-Closer Soft Shadows（PCSS）

```text
软阴影技术
根据遮挡物距离计算半影大小

优势：真实的软阴影
劣势：性能开销大
```

---

## 典型使用示例

### 1. 完整光照设置

```rust
use bevy::prelude::*;
use bevy_light::prelude::*;
use bevy_light::light_consts;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    // 相机
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(-3.0, 3.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });
    
    // 方向光（太阳光）
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            color: Color::rgb(1.0, 0.95, 0.9),
            illuminance: light_consts::lux::DIRECT_SUNLIGHT,
            shadow_maps_enabled: true,
            ..default()
        },
        transform: Transform::from_rotation(Quat::from_euler(
            EulerRot::ZYX, 0.0, 1.0, -std::f32::consts::FRAC_PI_4
        )),
        ..default()
    });
    
    // 点光源（台灯）
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            color: Color::rgb(1.0, 0.9, 0.7),
            intensity: 5000.0,
            range: 5.0,
            radius: 0.05,
            shadow_maps_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(0.0, 1.5, 0.0),
        ..default()
    });
    
    // 聚光灯（手电筒）
    commands.spawn(SpotLightBundle {
        spot_light: SpotLight {
            color: Color::rgb(1.0, 0.95, 0.8),
            intensity: 10000.0,
            range: 8.0,
            outer_angle: std::f32::consts::FRAC_PI_6,
            inner_angle: std::f32::consts::FRAC_PI_12,
            shadow_maps_enabled: true,
            ..default()
        },
        transform: Transform {
            translation: Vec3::new(2.0, 2.0, 2.0),
            rotation: Quat::from_rotation_x(-0.5),
            ..default()
        },
        ..default()
    });
    
    // 环境光
    commands.insert_resource(GlobalAmbientLight {
        color: Color::rgb(0.3, 0.4, 0.5),
        brightness: 200.0,
    });
    
    // 环境贴图
    commands.spawn(EnvironmentMapLight {
        environment_map: asset_server.load("environment.hdr"),
        brightness: 500.0,
        background: EnvironmentMapBackground::None,
    });
    
    // 地面
    commands.spawn(PbrBundle {
        mesh: asset_server.load("models/ground.glb#Mesh0/Primitive0"),
        material: asset_server.load("materials/ground.StandardMaterial"),
        ..default()
    });
}
```

### 2. 高级阴影配置

```rust
use bevy::prelude::*;
use bevy_light::{DirectionalLight, CascadeShadowConfigBuilder};

fn setup_advanced_shadows(mut commands: Commands) {
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            color: Color::WHITE,
            illuminance: 100000.0,
            shadow_maps_enabled: true,
            shadow_depth_bias: 0.05,
            shadow_normal_bias: 2.0,
            ..default()
        },
        cascade_shadow_config: CascadeShadowConfigBuilder {
            first_cascade_far_bound: 4.0,
            maximum_distance: 50.0,
            cascade_count: 4,
            ..default()
        }.into(),
        transform: Transform::from_rotation(Quat::from_euler(
            EulerRot::ZYX, 0.0, 1.0, -0.785
        )),
        ..default()
    });
    
    // 高分辨率阴影贴图
    commands.insert_resource(DirectionalLightShadowMap { size: 8192 });
    commands.insert_resource(PointLightShadowMap { size: 2048 });
}
```

### 3. 体积光和雾

```rust
use bevy::prelude::*;
use bevy_light::{PointLight, VolumetricLight};
use bevy_pbr::VolumetricFogPlugin;

fn setup_volumetric_effects(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    // 体积点光源
    commands.spawn((
        PointLightBundle {
            point_light: PointLight {
                color: Color::rgb(1.0, 0.8, 0.6),
                intensity: 20000.0,
                range: 15.0,
                ..default()
            },
            transform: Transform::from_xyz(0.0, 3.0, 0.0),
            ..default()
        },
        VolumetricLight {
            scattering_coefficient: 0.1,
            absorption_coefficient: 0.01,
            anisotropy: 0.9,
        },
    ));
    
    // 体积雾
    commands.spawn(PbrBundle {
        mesh: asset_server.load("models/fog_volume.glb#Mesh0/Primitive0"),
        material: asset_server.load("materials/transparent.StandardMaterial"),
        transform: Transform::from_xyz(0.0, 0.5, 0.0),
        ..default()
    });
}
```

---

## 性能优化

### 1. 光源数量

```rust
// 限制动态光源数量
// 方向光：1-2 个
// 点光源：< 10 个（无分块），< 1000 个（有分块）
// 聚光灯：< 5 个
```

### 2. 阴影优化

```rust
// 使用合适的阴影分辨率
DirectionalLightShadowMap { size: 2048 }  // 中等场景
DirectionalLightShadowMap { size: 4096 }  // 高质量

// 限制阴影距离
CascadeShadowConfig {
    maximum_distance: 50.0,  // 只在 50 米内渲染阴影
    ..default()
}

// 减少级联数量
CascadeShadowConfig {
    cascade_count: 2,  // 2 级联（性能） vs 4 级联（质量）
    ..default()
}
```

### 3. 分块光照

```rust
// 使用分块光照支持大量光源
ClusterConfig::XYZ {
    dimensions: UVec3::new(16, 16, 24),
    ..default()
}

// 动态调整分块大小
ClusterConfig::XYZ {
    dynamic_resizing: true,  // 自动避免分块溢出
    ..default()
}
```

### 4. 视锥剔除

```rust
// Bevy 自动进行视锥剔除
// 确保光源有正确的 range
PointLight {
    range: 10.0,  // 只影响 10 米内
    ..default()
}
```

---

## 设计特点

### 1. 物理准确
- **真实单位**：lux、lumens 等物理单位
- **能量守恒**：光照计算符合物理规律
- **基于 PBR**：与 bevy_pbr 无缝集成

### 2. 可扩展
- **自定义光源**：支持实现自定义光源类型
- **自定义阴影**：支持自定义阴影技术
- **模块化**：各个组件独立，易于替换

### 3. 高性能
- **分块光照**：支持数千个光源
- **视锥剔除**：只计算可见光源
- **LOD**：阴影贴图级联

### 4. 易用性
- **Bundle**：预定义的光源 Bundle
- **默认值**：合理的默认参数
- **构建器**：CascadeShadowConfigBuilder

---

## 文件结构

```
src/
├── directional_light.rs       # 方向光
├── point_light.rs             # 点光源
├── spot_light.rs              # 聚光灯
├── ambient_light.rs           # 环境光
├── cascade.rs                 # 级联阴影
├── cluster/                   # 分块光照
│   ├── mod.rs
│   ├── assign.rs              # 光源分配
│   └── test.rs
├── probe.rs                   # 光照探针
├── volumetric.rs              # 体积光
└── lib.rs                     # 主入口和 LightPlugin
```

---

## 总结

`bevy_light` 是一个**功能完整、物理准确的光照系统**，具有以下优势：

**核心优势**：
1. **多种光源类型**：方向光、点光源、聚光灯、环境光
2. **高级阴影**：CSM、立方体阴影、接触阴影、PCSS
3. **分块光照**：支持数千个动态光源
4. **物理准确**：基于真实物理单位
5. **可扩展**：易于添加自定义光源和阴影技术

**适用场景**：
- 3D 游戏开发
- 建筑可视化
- 电影渲染
- 科学可视化

**学习资源**：
- [Bevy Light 文档](https://docs.rs/bevy/latest/bevy/light/index.html)
- [Real-time Rendering Book](https://www.realtimerendering.com/)
- [Physically Based Rendering Book](https://www.pbr-book.org/)

---

**注意**：`bevy_light` 是底层光照系统，与 `bevy_pbr` 紧密集成。理解光照系统对于创建高质量渲染至关重要。
