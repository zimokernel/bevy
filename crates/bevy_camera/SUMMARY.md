# bevy_camera 模块总结

## 概述

`bevy_camera` 是 Bevy 游戏引擎的**相机系统核心模块**，提供了完整的 3D/2D 相机功能、投影系统、可见性管理和渲染目标支持。它是连接游戏世界和渲染输出的关键桥梁。

**核心特性**：
- **灵活的相机系统**：支持 3D 透视相机和 2D 正交相机
- **多种投影模式**：透视投影、正交投影、自定义投影
- **高级可见性管理**：视锥体剔除、渲染层、可见范围
- **多目标渲染**：支持窗口、纹理、自定义纹理视图
- **视口系统**：支持分屏、小地图、多显示器设置
- **曝光控制**：支持摄影级曝光值 (EV100)

---

## 核心架构

```
Camera System（相机系统）
├── Camera Component（相机组件）
│   ├── Viewport（视口）
│   ├── Render Target（渲染目标）
│   ├── Projection（投影）
│   └── Output Mode（输出模式）
├── Projection System（投影系统）
│   ├── Perspective（透视）
│   ├── Orthographic（正交）
│   └── Custom（自定义）
├── Visibility System（可见性系统）
│   ├── Frustum Culling（视锥体剔除）
│   ├── Render Layers（渲染层）
│   └── Visibility Range（可见范围）
└── Camera Controller（相机控制器）
    ├── Transform（变换）
    └── GlobalTransform（全局变换）
```

**关键设计**：
- **ECS 架构**：相机作为组件，支持多个相机实体
- **分离关注点**：投影、可见性、渲染目标独立管理
- **可扩展性**：支持自定义投影和渲染目标
- **性能优化**：视锥体剔除、可见性缓存

---

## 主要子模块

### 模块功能总览

| 模块 | 功能 | 关键组件 |
|------|------|----------|
| **camera.rs** | 核心相机组件 | `Camera`, `Viewport`, `RenderTarget`, `Exposure` |
| **projection.rs** | 投影系统 | `Projection`, `PerspectiveProjection`, `OrthographicProjection` |
| **components.rs** | 相机类型组件 | `Camera2d`, `Camera3d`, `Camera3dDepthLoadOp` |
| **visibility/** | 可见性管理 | `Visibility`, `RenderLayers`, `Frustum` |
| **primitives.rs** | 图元 | `Frustum`, `Aabb`, `Sphere` |
| **clear_color.rs** | 清除颜色 | `ClearColor`, `ClearColorConfig` |

---

## 核心子模块详解

### 1. Camera 组件

**文件**: [`camera.rs`](file:///d:/work/ttc/bevy/crates/bevy_camera/src/camera.rs)

#### Camera 结构定义

```rust
#[derive(Component, Debug, Reflect, Clone)]
pub struct Camera {
    /// 视口配置（可选）
    pub viewport: Option<Viewport>,
    
    /// 渲染顺序（值越大越晚渲染，在上面）
    pub order: isize,
    
    /// 是否激活（false 时不渲染）
    pub is_active: bool,
    
    /// 计算值（投影矩阵、渲染目标大小等）
    #[reflect(ignore, clone)]
    pub computed: ComputedCameraValues,
    
    /// 输出模式
    pub output_mode: CameraOutputMode,
    
    /// MSAA 回写控制
    pub msaa_writeback: MsaaWriteback,
    
    /// 清除颜色配置
    pub clear_color: ClearColorConfig,
    
    /// 是否反转剔除模式
    pub invert_culling: bool,
    
    /// 子相机视图（用于多显示器）
    pub sub_camera_view: Option<SubCameraView>,
}
```

**字段说明**：

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `viewport` | `Option<Viewport>` | `None` | 自定义视口区域 |
| `order` | `isize` | `0` | 渲染顺序，大值在后 |
| `is_active` | `bool` | `true` | 是否激活渲染 |
| `computed` | `ComputedCameraValues` | - | 内部计算值 |
| `output_mode` | `CameraOutputMode` | `Write` | 输出模式 |
| `msaa_writeback` | `MsaaWriteback` | `Auto` | MSAA 控制 |
| `clear_color` | `ClearColorConfig` | `Default` | 清除颜色 |
| `invert_culling` | `bool` | `false` | 反转剔除 |
| `sub_camera_view` | `Option<SubCameraView>` | `None` | 子相机视图 |

#### Viewport 结构

```rust
#[derive(Reflect, Debug, Clone)]
pub struct Viewport {
    /// 物理位置（左上角为原点）
    pub physical_position: UVec2,
    
    /// 物理大小
    pub physical_size: UVec2,
    
    /// 深度范围（0.0 到 1.0）
    pub depth: Range<f32>,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            physical_position: Default::default(),
            physical_size: UVec2::new(1, 1),
            depth: 0.0..1.0,
        }
    }
}
```

**视口使用场景**：
- **分屏游戏**：多个玩家在同一屏幕
- **小地图**：角落显示缩小的地图
- **多显示器**：跨多个显示器渲染
- **画中画**：主画面中的小窗口

#### RenderTarget 枚举

```rust
#[derive(Component, Debug, Clone, Reflect, From)]
pub enum RenderTarget {
    /// 渲染到窗口
    Window(WindowRef),
    
    /// 渲染到纹理
    Image(ImageRenderTarget),
    
    /// 渲染到手动创建的纹理视图
    TextureView(ManualTextureViewHandle),
    
    /// 不渲染到任何颜色目标（仅深度预传递）
    None {
        size: UVec2,
    },
}
```

**渲染目标类型**：

| 类型 | 用途 | 示例 |
|------|------|------|
| `Window` | 主窗口渲染 | 游戏主画面 |
| `Image` | 离屏渲染 | 截图、后处理 |
| `TextureView` | 外部纹理 | OpenXR、自定义纹理 |
| `None` | 深度预传递 | 仅渲染深度 |

#### ComputedCameraValues 结构

```rust
#[derive(Default, Debug, Clone)]
pub struct ComputedCameraValues {
    /// 投影矩阵（裁剪空间到视图空间）
    pub clip_from_view: Mat4,
    
    /// 渲染目标信息
    pub target_info: Option<RenderTargetInfo>,
    
    /// 旧视口大小（用于检测变化）
    pub old_viewport_size: Option<UVec2>,
    
    /// 旧子相机视图（用于检测变化）
    pub old_sub_camera_view: Option<SubCameraView>,
}
```

**设计目的**：
- **缓存计算结果**：避免重复计算投影矩阵
- **变化检测**：通过比较旧值检测视口变化
- **渲染优化**：仅在必要时重新计算

#### Exposure 组件

```rust
#[derive(Component, Clone, Copy, Reflect)]
pub struct Exposure {
    /// 曝光值（EV100）
    pub ev100: f32,
}

impl Exposure {
    // 预设值
    pub const SUNLIGHT: Self = Self { ev100: 15.0 };
    pub const OVERCAST: Self = Self { ev100: 12.0 };
    pub const INDOOR: Self = Self { ev100: 7.0 };
    pub const BLENDER: Self = Self { ev100: 9.7 };
}
```

**曝光值说明**：
- **EV100**：国际标准曝光值
- **15.0**：阳光充足（f/16, 1/125s, ISO 100）
- **12.0**：阴天（f/16, 1/60s, ISO 100）
- **7.0**：室内（f/2.8, 1/60s, ISO 100）
- **9.7**：Blender 默认（匹配 Blender 渲染）

**使用场景**：
- **物理光照**：配合 PBR 材质实现真实光照
- **HDR 渲染**：支持高动态范围渲染
- **后期处理**：曝光调整、色调映射

#### SubCameraView 结构

```rust
#[derive(Debug, Clone, Copy, Reflect, PartialEq)]
pub struct SubCameraView {
    /// 完整视图大小
    pub full_size: UVec2,
    
    /// 子视图偏移
    pub offset: Vec2,
    
    /// 子视图大小
    pub size: UVec2,
}
```

**多显示器示例**：

```text
四显示器布局（每个 1920x1080）：
┌───┬───┐
│ A │ B │  完整大小：3840x2160
├───┼───┤
│ C │ D │
└───┴───┘

相机 A: full_size=(3840,2160), offset=(0,0), size=(1920,1080)
相机 B: full_size=(3840,2160), offset=(1920,0), size=(1920,1080)
相机 C: full_size=(3840,2160), offset=(0,1080), size=(1920,1080)
相机 D: full_size=(3840,2160), offset=(1920,1080), size=(1920,1080)
```

---

### 2. 投影系统

**文件**: [`projection.rs`](file:///d:/work/ttc/bevy/crates/bevy_camera/src/projection.rs)

#### Projection 枚举

```rust
#[derive(Component, Debug, Clone, Reflect, From)]
pub enum Projection {
    Perspective(PerspectiveProjection),
    Orthographic(OrthographicProjection),
    Custom(CustomProjection),
}

impl Default for Projection {
    fn default() -> Self {
        Projection::Perspective(Default::default())
    }
}
```

**投影类型**：

| 类型 | 用途 | 特点 |
|------|------|------|
| `Perspective` | 3D 游戏 | 近大远小 |
| `Orthographic` | 2D 游戏/UI | 大小一致 |
| `Custom` | 特殊需求 | 完全自定义 |

#### CameraProjection 特质

```rust
pub trait CameraProjection {
    /// 获取投影矩阵
    fn get_clip_from_view(&self) -> Mat4;
    
    /// 获取子相机投影矩阵
    fn get_clip_from_view_for_sub(&self, sub_view: &SubCameraView) -> Mat4;
    
    /// 更新投影（视口变化时调用）
    fn update(&mut self, width: f32, height: f32);
    
    /// 获取远裁剪面距离
    fn far(&self) -> f32;
    
    /// 计算视锥体角点
    fn build_frustum_corners(&self, camera_transform: &GlobalTransform) -> [Vec3A; 8];
    
    /// 计算视锥体
    fn compute_frustum(&self, camera_transform: &GlobalTransform) -> Frustum;
    
    /// 视口坐标转 NDC
    fn viewport_to_ndc(&self, viewport_position: Vec2) -> Vec2;
    
    /// NDC 转视口坐标
    fn ndc_to_viewport(&self, ndc_position: Vec2) -> Vec2;
    
    /// 视口坐标转射线
    fn viewport_to_world(
        &self,
        camera_transform: &GlobalTransform,
        viewport_position: Vec2,
    ) -> Result<Ray3d, CameraProjectionError>;
    
    /// 视口坐标转 2D 世界坐标
    fn viewport_to_world_2d(
        &self,
        camera_transform: &GlobalTransform,
        viewport_position: Vec2,
    ) -> Result<Vec2, CameraProjectionError>;
    
    /// NDC 转世界坐标
    fn ndc_to_world(
        &self,
        camera_transform: &GlobalTransform,
        ndc_position: Vec2,
        depth: f32,
    ) -> Vec3;
}
```

**关键方法**：

| 方法 | 功能 | 用途 |
|------|------|------|
| `get_clip_from_view` | 获取投影矩阵 | 渲染管线 |
| `update` | 更新投影 | 窗口调整 |
| `compute_frustum` | 计算视锥体 | 可见性剔除 |
| `viewport_to_world` | 视口转射线 | 鼠标拾取 |
| `ndc_to_world` | NDC 转世界 | 坐标转换 |

#### PerspectiveProjection 结构

```rust
#[derive(Debug, Clone, Reflect)]
pub struct PerspectiveProjection {
    /// 垂直视场角（FOV），弧度
    pub fov: f32,  // 默认：π/4 (45°)
    
    /// 宽高比
    pub aspect_ratio: f32,  // 默认：1.0
    
    /// 近裁剪面距离
    pub near: f32,  // 默认：0.1
    
    /// 远裁剪面距离
    pub far: f32,  // 默认：1000.0
    
    /// 自定义裁剪平面
    pub custom_projection: Option<Mat4>,
}
```

**透视投影公式**：

```text
投影矩阵（右手坐标系，-Z 向前）：
[ 1/(tan(fov/2)*aspect)  0                    0                          0 ]
[ 0                      1/tan(fov/2)         0                          0 ]
[ 0                      0                    -(far+near)/(far-near)    -2*far*near/(far-near) ]
[ 0                      0                    -1                         0 ]

特性：
- 近大远小（真实透视）
- 适合 3D 游戏
- 支持大场景渲染
```

**注意事项**：
- **Reverse-Z**：Bevy 使用反向 Z（近=1.0，远=0.0）提高精度
- **精度问题**：`far/near` 比值过大会导致精度损失
- **建议**：使用 `0.1` 到 `1000.0` 的合理范围

#### OrthographicProjection 结构

```rust
#[derive(Debug, Clone, Reflect)]
pub struct OrthographicProjection {
    /// 近裁剪面
    pub near: f32,  // 默认：0.0
    
    /// 远裁剪面
    pub far: f32,  // 默认：1000.0
    
    /// 缩放模式
    pub scaling_mode: ScalingMode,  // 默认：WindowSize
    
    /// 缩放因子
    pub scale: f32,  // 默认：1.0
    
    /// 投影区域
    pub area: Rect,  // 自动计算
}
```

**缩放模式**：

```rust
#[derive(Default, Debug, Clone, Copy, Reflect)]
pub enum ScalingMode {
    /// 匹配窗口大小（1 世界单位 = 1 像素）
    WindowSize,
    
    /// 固定宽度
    FixedHorizontal { viewport_width: f32 },
    
    /// 固定高度
    FixedVertical { viewport_height: f32 },
    
    /// 固定大小（保持宽高比）
    Fixed { width: f32, height: f32 },
}
```

**正交投影公式**：

```text
投影矩阵：
[ 2/width   0         0          0 ]
[ 0         2/height  0          0 ]
[ 0         0         -2/(far-near)  -(far+near)/(far-near) ]
[ 0         0         0          1 ]

特性：
- 大小一致（无透视）
- 适合 2D 游戏和 UI
- 精确的坐标对齐
```

**使用示例**：

```rust
// 2D 相机（默认）
let projection = OrthographicProjection::default_2d();

// 固定高度（2 世界单位）
let projection = OrthographicProjection {
    scaling_mode: ScalingMode::FixedVertical { viewport_height: 2.0 },
    ..default()
};

// 固定大小（100x100 世界单位）
let projection = OrthographicProjection {
    scaling_mode: ScalingMode::Fixed { width: 100.0, height: 100.0 },
    ..default()
};
```

#### CustomProjection 结构

```rust
struct CustomProjection {
    dyn_projection: Box<dyn CameraProjection + Send + Sync>,
}

impl CustomProjection {
    /// 获取特定类型的投影
    pub fn get<P>(&self) -> Option<&P> where P: CameraProjection + 'static {
        self.dyn_projection.downcast_ref()
    }
    
    /// 获取可变引用
    pub fn get_mut<P>(&mut self) -> Option<&mut P> where P: CameraProjection + 'static {
        self.dyn_projection.downcast_mut()
    }
}
```

**自定义投影示例**：

```rust
// 实现自定义投影
struct MyProjection {
    // 自定义字段
}

impl CameraProjection for MyProjection {
    fn get_clip_from_view(&self) -> Mat4 {
        // 自定义投影矩阵
        Mat4::identity()
    }
    
    fn update(&mut self, width: f32, height: f32) {
        // 视口变化时更新
    }
    
    // 实现其他方法...
}

// 使用自定义投影
let camera = Camera3dBundle {
    projection: Projection::custom(MyProjection::default()),
    ..default()
};
```

---

### 3. 相机类型组件

**文件**: [`components.rs`](file:///d:/work/ttc/bevy/crates/bevy_camera/src/components.rs)

#### Camera2d 组件

```rust
#[derive(Component, Default, Reflect, Clone)]
pub struct Camera2d;
```

**特点**：
- 自动使用正交投影
- 适合 2D 游戏和 UI
- 简化的相机设置

**使用示例**：

```rust
commands.spawn(Camera2dBundle {
    transform: Transform::from_xyz(0.0, 0.0, 100.0),
    ..default()
});
```

#### Camera3d 组件

```rust
#[derive(Component, Reflect, Clone)]
pub struct Camera3d {
    /// 深度加载操作
    pub depth_load_op: Camera3dDepthLoadOp,
    
    /// 深度纹理使用
    pub depth_texture_usages: Camera3dDepthTextureUsage,
    
    /// 透射通道步数
    pub transmissive_pass_count: u32,
}
```

**字段说明**：

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `depth_load_op` | `Camera3dDepthLoadOp` | `Clear(0.0)` | 深度缓冲操作 |
| `depth_texture_usages` | `Camera3dDepthTextureUsage` | `empty()` | 深度纹理用途 |
| `transmissive_pass_count` | `u32` | `1` | 透射通道数 |

#### Camera3dDepthLoadOp 枚举

```rust
#[derive(Reflect, Serialize, Deserialize, Clone, Debug)]
pub enum Camera3dDepthLoadOp {
    /// 清除到指定值（0.0 是远裁剪面）
    Clear(f32),
    
    /// 从内存加载
    Load,
}
```

**使用场景**：
- **Clear(0.0)**：默认，每次渲染清除深度
- **Load**：保留上一帧深度（用于延迟渲染）

---

### 4. 可见性系统

**文件**: [`visibility/mod.rs`](file:///d:/work/ttc/bevy/crates/bevy_camera/src/visibility/mod.rs)

#### Visibility 组件

```rust
#[derive(Component, Clone, Copy, Reflect, PartialEq, Eq, Default)]
pub enum Visibility {
    /// 继承父实体可见性
    #[default]
    Inherited,
    
    /// 强制可见
    Visible,
    
    /// 强制不可见
    Hidden,
}
```

**可见性继承**：
- **Inherited**：继承父实体的可见性
- **Visible**：始终可见（忽略父实体）
- **Hidden**：始终不可见（忽略父实体）

#### InheritedVisibility 组件

```rust
#[derive(Component, Clone, Copy, Reflect, PartialEq, Eq, Default)]
pub enum InheritedVisibility {
    /// 可见（继承链中无 Hidden）
    #[default]
    Visible,
    
    /// 不可见（继承链中有 Hidden）
    Hidden,
}
```

**自动更新**：由系统自动计算，无需手动设置

#### ViewVisibility 组件

```rust
#[derive(Component, Clone, Copy, Reflect, PartialEq, Eq, Default)]
pub enum ViewVisibility {
    /// 对所有相机可见
    #[default]
    Visible,
    
    /// 对某些相机不可见
    HiddenInSomeViews,
    
    /// 对所有相机不可见
    Hidden,
}
```

**相机可见性**：
- **Visible**：在所有相机视锥体内
- **HiddenInSomeViews**：在部分相机视锥体外
- **Hidden**：在所有相机视锥体外

#### RenderLayers 组件

```rust
#[derive(Component, Clone, Reflect, PartialEq, Eq, PartialOrd, Ord)]
pub struct RenderLayers(SmallVec<[u64; INLINE_BLOCKS]>);

impl RenderLayers {
    /// 创建包含单个层的 RenderLayers
    pub fn layer(layer: Layer) -> Self {
        // layer: 0-63
    }
    
    /// 创建包含多个层的 RenderLayers
    pub fn layers(layers: impl IntoIterator<Item = Layer>) -> Self {
        // layers: 多个层
    }
    
    /// 检查是否包含层
    pub fn contains(&self, layer: Layer) -> bool {
        // 检查层是否在集合中
    }
    
    /// 检查是否与另一组层相交
    pub fn overlaps(&self, other: &RenderLayers) -> bool {
        // 检查是否有共同层
    }
}
```

**使用示例**：

```rust
// 相机渲染层 0 和 1
let camera = Camera3dBundle {
    camera: Camera {
        // 相机默认渲染所有层
        ..default()
    },
    ..default()
};

// 实体仅在层 0
let entity = commands.spawn((
    PbrBundle { ..default() },
    RenderLayers::layer(0),
));

// 实体在层 0 和 1
let entity = commands.spawn((
    PbrBundle { ..default() },
    RenderLayers::layers([0, 1]),
));
```

**渲染规则**：
- 相机渲染所有层（默认）
- 实体仅在其层与相机层相交时渲染
- 空 `RenderLayers` 使实体不可见

#### Frustum 结构

```rust
#[derive(Clone, Copy, Debug)]
pub struct Frustum {
    /// 6 个裁剪平面（左、右、上、下、近、远）
    pub planes: [Plane3d; 6],
    
    /// 视锥体角点
    pub corners: [Vec3A; 8],
}

impl Frustum {
    /// 检查点是否在视锥体内
    pub fn contains_point(&self, point: Vec3A) -> bool {
        // 检查是否在所有平面正面
    }
    
    /// 检查 AABB 是否与视锥体相交
    pub fn intersects_aabb(&self, aabb: &Aabb) -> bool {
        // AABB-Frustum 相交测试
    }
    
    /// 检查球体是否与视锥体相交
    pub fn intersects_sphere(&self, sphere: &Sphere) -> bool {
        // Sphere-Frustum 相交测试
    }
}
```

**视锥体剔除**：

```text
视锥体形状（透视）：
        近裁剪面
        ┌─────┐
       /     /|
      /  视  / |  远裁剪面
     /  锥  /  |   ┌─────┐
    └─────┘   |  /     /
     相机     | /  体  /
              |/     /
              └─────┘

剔除测试：
1. 点测试：检查点是否在所有平面正面
2. AABB 测试：使用分离轴定理
3. 球体测试：检查球心到平面距离

性能优化：
- 层级剔除：先剔除父实体
- 空间划分：使用 BVH 或八叉树
- 缓存结果：避免重复测试
```

---

### 5. 清除颜色系统

**文件**: [`clear_color.rs`](file:///d:/work/ttc/bevy/crates/bevy_camera/src/clear_color.rs)

#### ClearColor 资源

```rust
#[derive(Resource, Clone, Deref, DerefMut, Reflect)]
pub struct ClearColor(pub Color);

impl Default for ClearColor {
    fn default() -> Self {
        Self(Color::srgb(0.1, 0.1, 0.1))
    }
}
```

**全局清除颜色**：
- 默认：深灰色 (0.1, 0.1, 0.1)
- 可在 `App` 中设置
- 可被相机覆盖

#### ClearColorConfig 枚举

```rust
#[derive(Reflect, Serialize, Deserialize, Copy, Clone, Debug, Default, From)]
pub enum ClearColorConfig {
    /// 使用全局 ClearColor 资源
    #[default]
    Default,
    
    /// 使用自定义颜色
    Custom(Color),
    
    /// 不清除（在已有内容上绘制）
    None,
}
```

**使用示例**：

```rust
// 设置全局清除颜色
app.insert_resource(ClearColor(Color::srgb(0.0, 0.2, 0.4)));

// 相机使用自定义颜色
let camera = Camera3dBundle {
    camera: Camera {
        clear_color: ClearColorConfig::Custom(Color::WHITE),
        ..default()
    },
    ..default()
};

// 相机不清除（叠加渲染）
let overlay_camera = Camera3dBundle {
    camera: Camera {
        clear_color: ClearColorConfig::None,
        order: 1,  // 在主相机之后渲染
        ..default()
    },
    ..default()
};
```

---

## 典型使用示例

### 1. 创建 3D 透视相机

```rust
use bevy::prelude::*;

fn setup(mut commands: Commands) {
    // 创建 3D 相机
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(0.0, 2.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        projection: Projection::Perspective(PerspectiveProjection {
            fov: std::f32::consts::PI / 4.0,  // 45°
            near: 0.1,
            far: 1000.0,
            ..default()
        }),
        camera: Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.1, 0.1, 0.1)),
            ..default()
        },
        ..default()
    });
}
```

### 2. 创建 2D 正交相机

```rust
use bevy::prelude::*;

fn setup(mut commands: Commands) {
    // 创建 2D 相机
    commands.spawn(Camera2dBundle {
        transform: Transform::from_xyz(0.0, 0.0, 100.0),
        projection: Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical { viewport_height: 2.0 },
            scale: 1.0,
            ..default()
        }),
        camera: Camera {
            clear_color: ClearColorConfig::Custom(Color::WHITE),
            ..default()
        },
        ..default()
    });
}
```

### 3. 分屏相机

```rust
use bevy::prelude::*;

fn setup_split_screen(mut commands: Commands) {
    // 左半屏相机（玩家 1）
    commands.spawn(Camera3dBundle {
        camera: Camera {
            viewport: Some(Viewport {
                physical_position: UVec2::new(0, 0),
                physical_size: UVec2::new(400, 600),
                depth: 0.0..1.0,
            }),
            order: 0,
            ..default()
        },
        transform: Transform::from_xyz(-5.0, 2.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });
    
    // 右半屏相机（玩家 2）
    commands.spawn(Camera3dBundle {
        camera: Camera {
            viewport: Some(Viewport {
                physical_position: UVec2::new(400, 0),
                physical_size: UVec2::new(400, 600),
                depth: 0.0..1.0,
            }),
            order: 0,
            ..default()
        },
        transform: Transform::from_xyz(5.0, 2.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });
}
```

### 4. 小地图相机

```rust
use bevy::prelude::*;

fn setup_minimap(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    // 创建渲染目标
    let size = Extent3d {
        width: 256,
        height: 256,
        ..default()
    };
    
    let mut image = Image::new_fill(
        size,
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
    );
    image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT;
    let image_handle = images.add(image);
    
    // 小地图相机（俯视）
    commands.spawn(Camera3dBundle {
        camera: Camera {
            target: RenderTarget::Image(image_handle.clone()),
            order: 1,
            ..default()
        },
        transform: Transform::from_xyz(0.0, 100.0, 0.0).looking_at(Vec3::ZERO, Vec3::NEG_Y),
        projection: Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::Fixed { width: 200.0, height: 200.0 },
            ..default()
        }),
        ..default()
    });
    
    // 小地图 UI（显示渲染结果）
    commands.spawn(NodeBundle {
        style: Style {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            right: Val::Px(10.0),
            width: Val::Px(256.0),
            height: Val::Px(256.0),
            ..default()
        },
        ..default()
    }).with_children(|parent| {
        parent.spawn(ImageBundle {
            image: UiImage::new(image_handle),
            ..default()
        });
    });
}
```

### 5. 鼠标拾取

```rust
use bevy::prelude::*;
use bevy_math::Ray3d;

fn mouse_pick(
    camera_query: Query<(&Camera, &GlobalTransform)>,
    window: Query<&Window>,
    meshes: Query<&Handle<Mesh>>,
) {
    let (camera, camera_transform) = camera_query.single();
    let window = window.single();
    
    if let Some(cursor_pos) = window.cursor_position() {
        // 视口坐标转射线
        if let Ok(ray) = camera.viewport_to_world(camera_transform, cursor_pos) {
            // 使用射线检测碰撞
            println!("Ray origin: {}, direction: {}", ray.origin, ray.direction);
            
            // 与网格相交测试...
        }
    }
}
```

---

## 相机更新流程

### 系统调度

```text
帧更新流程：
1. TransformSystems::Propagate
   - 更新全局变换
   - 计算父实体变换

2. CameraProjectionPlugin
   - 更新投影矩阵
   - 计算视锥体
   - 更新可见性

3. VisibilityPlugin
   - 视锥体剔除
   - 渲染层过滤
   - 可见范围检查

4. Render Systems
   - 渲染可见实体
   - 应用后处理
   - 输出到目标
```

### 关键系统

```rust
// 相机投影更新系统
fn camera_projection_system<T: CameraProjection>(
    mut query: Query<(&GlobalTransform, &mut Projection, &Camera), With<T>>,
) {
    // 1. 检查视口是否变化
    // 2. 更新投影矩阵
    // 3. 计算视锥体
    // 4. 更新 computed 字段
}

// 可见性更新系统
fn check_visibility_system(
    mut views: Query<(&ViewFrustum, &mut VisibleEntities)>,
    entities: Query<(&GlobalTransform, &Aabb, &RenderLayers)>,
) {
    // 1. 遍历所有相机
    // 2. 遍历所有实体
    // 3. 视锥体剔除测试
    // 4. 渲染层过滤
    // 5. 更新 VisibleEntities
}
```

---

## 设计特点

### 1. ECS 架构
- **组件化**：相机由多个组件组成
- **灵活性**：支持多个相机实体
- **可组合性**：轻松添加新功能

### 2. 分离关注点
- **投影独立**：可独立更换投影模式
- **可见性独立**：可独立配置可见性
- **渲染目标独立**：可独立配置输出目标

### 3. 性能优化
- **视锥体剔除**：减少渲染实体数量
- **可见性缓存**：避免重复计算
- **变化检测**：仅在必要时更新

### 4. 可扩展性
- **自定义投影**：实现 `CameraProjection` 特质
- **自定义目标**：使用 `TextureView`
- **自定义可见性**：扩展可见性系统

---

## 常见问题

### 1. 相机不渲染

**可能原因**：
- `is_active` 为 `false`
- 缺少 `Camera2d` 或 `Camera3d` 组件
- 视锥体剔除（实体在视锥体外）
- 渲染层不匹配

**解决方法**：
```rust
// 检查相机是否激活
assert!(camera.is_active);

// 检查是否有相机类型组件
assert!(query.get::<Camera3d>(entity).is_ok());

// 检查渲染层
assert!(camera_render_layers.overlaps(&entity_render_layers));
```

### 2. 透视投影变形

**可能原因**：
- 宽高比未正确更新
- FOV 设置不合理
- 近/远裁剪面设置不当

**解决方法**：
```rust
// 确保宽高比正确
let projection = PerspectiveProjection {
    aspect_ratio: window.width() / window.height(),
    ..default()
};

// 合理的 FOV（45°-60°）
let projection = PerspectiveProjection {
    fov: std::f32::consts::PI / 4.0,  // 45°
    ..default()
};
```

### 3. 深度冲突

**可能原因**：
- 近裁剪面太近
- 远裁剪面太远
- 精度不足（Reverse-Z 问题）

**解决方法**：
```rust
// 使用合理的近/远裁剪面
let projection = PerspectiveProjection {
    near: 0.1,
    far: 1000.0,
    ..default()
};

// 避免过大的 far/near 比值
// 推荐：10000:1 以内
```

### 4. 多相机渲染顺序

**问题**：多个相机渲染顺序不正确

**解决方法**：
```rust
// 使用 order 字段控制渲染顺序
let background_camera = Camera3dBundle {
    camera: Camera { order: 0, ..default() },
    ..default()
};

let foreground_camera = Camera3dBundle {
    camera: Camera { order: 1, ..default() },
    ..default()
};
```

---

## 性能优化建议

### 1. 视锥体剔除
```rust
// 启用视锥体剔除（默认启用）
// 确保实体有 AABB 组件
commands.spawn((
    PbrBundle { ..default() },
    Aabb::from_min_max(Vec3::ZERO, Vec3::ONE),
));
```

### 2. 渲染层过滤
```rust
// 使用渲染层减少测试实体
let camera = Camera3dBundle {
    camera: Camera {
        // 相机默认渲染所有层
        ..default()
    },
    ..default()
};

// 实体仅在特定层
commands.spawn((
    PbrBundle { ..default() },
    RenderLayers::layer(0),
));
```

### 3. 合理的近/远裁剪面
```rust
// 避免过大的范围
let projection = PerspectiveProjection {
    near: 0.1,
    far: 1000.0,  // 不要设置为 1000000.0
    ..default()
};
```

### 4. 减少相机数量
```rust
// 避免过多相机
// 每个相机都需要视锥体剔除
// 考虑使用单个相机 + 多视口
```

---

## 文件结构

```
src/
├── camera.rs                    # 核心相机组件
├── projection.rs                # 投影系统
├── components.rs                # 相机类型组件
├── clear_color.rs               # 清除颜色系统
├── primitives.rs                # 图元（Frustum, Aabb 等）
├── visibility/                  # 可见性系统
│   ├── mod.rs                   # 可见性主文件
│   ├── render_layers.rs         # 渲染层
│   └── range.rs                 # 可见范围
└── lib.rs                       # 主入口和 CameraPlugin
```

---

## 总结

`bevy_camera` 是一个**功能完整、灵活高效的相机系统**，具有以下优势：

**核心优势**：
1. **灵活的相机模型**：支持 3D/2D、透视/正交、多目标
2. **高级可见性管理**：视锥体剔除、渲染层、可见范围
3. **可扩展的投影系统**：支持自定义投影实现
4. **多目标渲染**：窗口、纹理、自定义纹理视图
5. **性能优化**：视锥体剔除、可见性缓存、变化检测

**适用场景**：
- 3D 游戏（透视相机）
- 2D 游戏（正交相机）
- VR/AR（多相机、自定义纹理）
- 编辑器（多视口、小地图）
- 可视化（科学计算、数据可视化）

**学习资源**：
- [Bevy Camera 文档](https://docs.rs/bevy/latest/bevy/camera/index.html)
- [Bevy 示例](https://github.com/bevyengine/bevy/tree/main/examples)
- [Real-time Rendering Book](https://www.realtimerendering.com/)

---

**注意**：`bevy_camera` 是渲染系统的基础，与 `bevy_render` 和 `bevy_pbr` 紧密集成。理解相机系统对于优化渲染性能和实现高级渲染效果至关重要。
