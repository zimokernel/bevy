# bevy_core_pipeline 模块总结

## 概述

`bevy_core_pipeline` 是 Bevy 游戏引擎的**核心渲染管线模块**，提供了 2D 和 3D 场景的基础渲染能力。它是 Bevy 渲染系统的基础组件，负责组织和执行渲染流程中的各个阶段。

---

## 核心结构

### 主入口

**文件**: [`lib.rs`](file:///d:/work/ttc/bevy/crates/bevy_core_pipeline/src/lib.rs)

```rust
pub struct CorePipelinePlugin;

impl Plugin for CorePipelinePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            Core2dPlugin, Core3dPlugin,
            BlitPlugin, TonemappingPlugin,
            UpscalingPlugin, OrderIndependentTransparencyPlugin,
            MipGenerationPlugin
        ));
    }
}
```

**核心职责**：
- 注册所有子插件到 Bevy 应用
- 初始化全屏着色器资源
- 设置渲染图和相机驱动系统

---

## 主要子模块

| 模块 | 功能 | 关键组件 |
|------|------|----------|
| **core_2d** | 2D 场景渲染管线 | `Core2dPlugin`, `Opaque2d`, `Transparent2d` |
| **core_3d** | 3D 场景渲染管线 | `Core3dPlugin`, `Opaque3d`, `Transparent3d` |
| **prepass** | 预渲染通道 | `DepthPrepass`, `NormalPrepass`, `MotionVectorPrepass` |
| **tonemapping** | 色调映射（色彩校正） | `TonemappingPlugin`, 多种色调映射算法 |
| **upscaling** | 图像放大/超分辨率 | `UpscalingPlugin` |
| **oit** | 顺序无关透明度 | `OrderIndependentTransparencyPlugin` |
| **blit** | 图像复制/渲染到纹理 | `BlitPlugin` |
| **mip_generation** | Mipmap 自动生成 | `MipGenerationPlugin` |
| **deferred** | 延迟渲染支持 | `DeferredPrepass`, G-Buffer 管理 |
| **schedule** | 渲染调度系统 | `Core3d`, `Core2d` schedules |

---

## 核心渲染流程

### 3D 渲染管线

**文件**: [`core_3d/mod.rs`](file:///d:/work/ttc/bevy/crates/bevy_core_pipeline/src/core_3d/mod.rs)

```
[Prepasses] → [Main Pass] → [Post Processing] → [Output]
    ↓              ↓                ↓
  Depth        Opaque           Tonemapping
  Normal     Transparent        Upscaling
  Motion     Transmissive       ...
  Deferred
```

**关键渲染阶段**：
- **Opaque3d**：不透明物体渲染
- **AlphaMask3d**：Alpha 遮罩物体
- **Transmissive3d**：透射物体
- **Transparent3d**：透明物体（按深度排序）

### 2D 渲染管线

**文件**: [`core_2d/mod.rs`](file:///d:/work/ttc/bevy/crates/bevy_core_pipeline/src/core_2d/mod.rs)

```
[Main Pass] → [Post Processing]
    ↓              ↓
  Opaque2d     Tonemapping
  AlphaMask2d  Upscaling
  Transparent2d
```

---

## 调度系统

**文件**: [`schedule.rs`](file:///d:/work/ttc/bevy/crates/bevy_core_pipeline/src/schedule.rs)

### 3D 调度阶段

```rust
pub enum Core3dSystems {
    EndPrepasses,              // 预渲染完成
    StartMainPass,             // 开始主渲染
    EndMainPass,               // 主渲染完成
    StartMainPassPostProcessing,  // 开始后处理
    PostProcessing,            // 后处理中
    EndMainPassPostProcessing, // 后处理完成
}
```

### 2D 调度阶段

```rust
pub enum Core2dSystems {
    StartMainPass,
    EndMainPass,
    StartMainPassPostProcessing,
    PostProcessing,
    EndMainPassPostProcessing,
}
```

---

## 预渲染通道

**文件**: [`prepass/mod.rs`](file:///d:/work/ttc/bevy/crates/bevy_core_pipeline/src/prepass/mod.rs)

**可用的预渲染组件**：

```rust
// 添加到 Camera3d 实体以启用
DepthPrepass          // 深度缓冲
NormalPrepass         // 法线缓冲
MotionVectorPrepass   // 运动向量（用于运动模糊）
DeferredPrepass       // 延迟渲染 G-Buffer
```

**用途**：
- 减少主渲染通道的 overdraw
- 为屏幕空间效果提供数据（如 SSAO、SSR、运动模糊）

---

## 关键技术特性

### 1. 渲染阶段排序
- **Binned 渲染**：不透明物体按材质分桶，减少管线切换
- **Sorted 渲染**：透明物体按深度排序，保证正确混合

### 2. 多采样抗锯齿 (MSAA)
- 支持多种 MSAA 级别
- 自动处理深度和颜色缓冲的多采样

### 3. 色调映射
- 支持多种色调映射算法：
  - ACES (Academy Color Encoding System)
  - AgX
  - Filmic
  - Reinhard
  - 等...

### 4. 顺序无关透明度 (OIT)
- 处理复杂透明场景的高级技术
- 不需要严格的深度排序

---

## 典型使用方式

```rust
use bevy::prelude::*;
use bevy_core_pipeline::prepass::{DepthPrepass, NormalPrepass};

fn setup(mut commands: Commands) {
    // 创建启用预渲染的 3D 相机
    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(0.0, 2.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        },
        DepthPrepass,   // 启用深度预渲染
        NormalPrepass,  // 启用法线预渲染
    ));
}
```

---

## 设计特点

1. **插件化架构**：每个功能都是独立的 `Plugin`，可以按需启用
2. **渲染图驱动**：通过 `RenderGraph` 组织和执行渲染流程
3. **相机驱动**：每个相机可以配置不同的渲染特性
4. **可扩展**：用户可以添加自定义渲染阶段和后处理效果

---

## 文件结构

```
src/
├── core_2d/          # 2D 渲染管线
├── core_3d/          # 3D 渲染管线
├── prepass/          # 预渲染通道
├── tonemapping/      # 色调映射
├── upscaling/        # 图像放大
├── oit/              # 顺序无关透明度
├── blit/             # 图像复制
├── mip_generation/   # Mipmap 生成
├── deferred/         # 延迟渲染
├── schedule.rs       # 调度系统
├── fullscreen_material.rs
└── lib.rs            # 主入口
```

---

## 总结

`bevy_core_pipeline` 是 Bevy 渲染系统的**基础设施**，所有高级渲染功能（如 PBR、光照、阴影等）都构建在这个核心管线之上。它提供了灵活且高性能的渲染架构，支持从简单 2D 游戏到复杂 3D 场景的各种需求。

**关键优势**：
- 模块化设计，易于扩展
- 高性能的渲染流程组织
- 丰富的后处理和特效支持
- 灵活的相机配置系统
