# Bevy Anti-Alias 总结

## 1. 模块结构

`bevy_anti_alias` 是 Bevy 游戏引擎的抗锯齿模块，提供多种抗锯齿技术。主要模块包括：

- **fxaa**: 快速近似抗锯齿 (Fast Approximate Anti-Aliasing)
- **smaa**: 亚像素形态抗锯齿 (Subpixel Morphological Anti-Aliasing)
- **taa**: 时间抗锯齿 (Temporal Anti-Aliasing)
- **contrast_adaptive_sharpening**: 对比度自适应锐化 (Contrast Adaptive Sharpening)
- **dlss**: NVIDIA 深度学习超级采样 (NVIDIA Deep Learning Super Sampling) - 可选功能

## 2. 核心插件

### AntiAliasPlugin

主插件，添加所有抗锯齿支持：

```rust
/// Adds fxaa, smaa, taa, contrast aware sharpening, and optional dlss support.
#[derive(Default)]
pub struct AntiAliasPlugin;

impl Plugin for AntiAliasPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_plugins((
            FxaaPlugin,
            SmaaPlugin,
            TemporalAntiAliasPlugin,
            CasPlugin,
            #[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
            dlss::DlssPlugin,
        ));
    }
}
```

## 3. FXAA (Fast Approximate Anti-Aliasing)

### 核心概念

FXAA 是一种快速的单通道后处理抗锯齿技术，通过检测和模糊边缘来减少锯齿。

### Fxaa 组件

```rust
/// A component for enabling Fast Approximate Anti-Aliasing (FXAA)
/// for a [`bevy_camera::Camera`].
#[derive(Reflect, Component, Clone, ExtractComponent)]
pub struct Fxaa {
    /// Enable render passes for FXAA.
    pub enabled: bool,
    
    /// Use lower sensitivity for a sharper, faster, result.
    /// Use higher sensitivity for a slower, smoother, result.
    pub edge_threshold: Sensitivity,
    
    /// Trims the algorithm from processing darks.
    pub edge_threshold_min: Sensitivity,
}
```

### Sensitivity 枚举

```rust
#[derive(Debug, Reflect, Eq, PartialEq, Hash, Clone, Copy)]
pub enum Sensitivity {
    Low,
    Medium,
    High,
    Ultra,
    Extreme,
}
```

### 特点

- **优点**: 速度快，实现简单，对性能影响小
- **缺点**: 可能会模糊图像细节，特别是在高灵敏度设置下
- **适用场景**: 需要快速抗锯齿且对图像质量要求不是极高的场景

### 使用示例

```rust
commands.spawn((
    Camera3dBundle::default(),
    Fxaa {
        enabled: true,
        edge_threshold: Sensitivity::High,
        edge_threshold_min: Sensitivity::High,
    },
));
```

## 4. SMAA (Subpixel Morphological Anti-Aliasing)

### 核心概念

SMAA 是一种高质量的后处理抗锯齿技术，通过三个阶段处理图像：
1. **边缘检测**: 识别图像中的边缘
2. **混合权重计算**: 计算每个像素的混合权重
3. **邻域混合**: 根据混合权重混合像素

### Smaa 组件

```rust
/// A component for enabling Subpixel Morphological Anti-Aliasing (SMAA)
/// for a [`bevy_camera::Camera`].
#[derive(Clone, Copy, Default, Component, Reflect, ExtractComponent)]
pub struct Smaa {
    /// A predefined set of SMAA parameters: i.e. a quality level.
    pub preset: SmaaPreset,
}
```

### SmaaPreset 枚举

```rust
#[derive(Clone, Copy, Reflect, Default, PartialEq, Eq, Hash)]
pub enum SmaaPreset {
    /// Four search steps; no diagonal or corner detection.
    Low,
    
    /// Eight search steps; no diagonal or corner detection.
    Medium,
    
    /// Sixteen search steps, 8 diagonal search steps, and corner detection.
    #[default]
    High,
    
    /// Thirty-two search steps, 8 diagonal search steps, and corner detection.
    Ultra,
}
```

### 特点

- **优点**: 质量高，比 FXAA 更清晰，比 TAA 更稳定
- **缺点**: 性能开销比 FXAA 大，需要额外的查找表 (LUT) 纹理
- **适用场景**: 需要高质量抗锯齿且对性能要求不是极端严格的场景

### 使用示例

```rust
commands.spawn((
    Camera3dBundle::default(),
    Smaa {
        preset: SmaaPreset::High,
    },
    Msaa::Off, // SMAA 通常与 MSAA 不兼容
));
```

## 5. TAA (Temporal Anti-Aliasing)

### 核心概念

TAA 是一种时间抗锯齿技术，通过混合当前帧和过去几帧来减少锯齿。它利用运动向量来跟踪像素的运动，从而避免重影 artifacts。

### TemporalAntiAliasing 组件

```rust
/// Component to apply temporal anti-aliasing to a 3D camera.
#[derive(Component, Reflect, Clone)]
pub struct TemporalAntiAliasing {
    /// Set to true to delete the saved temporal history (past frames).
    /// Useful for preventing ghosting when the history is no longer
    /// representative of the current frame.
    pub reset: bool,
}
```

### 特点

**优点**:
- 过滤更多类型的锯齿，包括纹理和高光锯齿
- 成本随屏幕分辨率缩放，而不是三角形数量
- 大大提高随机渲染技术的质量（如 SSAO、某些阴影贴图采样方法等）

**缺点**:
- 可能出现"重影" - 移动物体留下的幽灵轨迹
- 细小的几何形状、光照细节或纹理线可能会闪烁或消失

### 使用注意事项

- 任何使用此组件的相机必须禁用 [`Msaa`]，设置为 [`Msaa::Off`]
- TAA 不适用于 alpha 混合的网格，因为它需要深度写入来确定运动
- 必须为屏幕上的所有内容正确写入运动向量，否则会导致重影 artifacts

### 使用示例

```rust
commands.spawn((
    Camera3dBundle::default(),
    TemporalAntiAliasing::default(),
    Msaa::Off, // TAA 必须禁用 MSAA
    DepthPrepass, // 需要深度预通道
    MotionVectorPrepass, // 需要运动向量预通道
    TemporalJitter::default(), // 需要时间抖动
    MipBias::default(), // 需要 MIP 偏差
));
```

## 6. CAS (Contrast Adaptive Sharpening)

### 核心概念

CAS 是一种对比度自适应锐化技术，通常与基于着色器的抗锯齿方法（如 FXAA 或 TAA）结合使用，以恢复它们引入的模糊导致的细节损失。

### ContrastAdaptiveSharpening 组件

```rust
/// Applies a contrast adaptive sharpening (CAS) filter to the camera.
#[derive(Component, Reflect, Clone)]
pub struct ContrastAdaptiveSharpening {
    /// Enable or disable sharpening.
    pub enabled: bool,
    
    /// Adjusts sharpening strength. Higher values increase the amount of sharpening.
    /// Clamped between 0.0 and 1.0.
    pub sharpening_strength: f32,
    
    /// Whether to try and avoid sharpening areas that are already noisy.
    pub denoise: bool,
}
```

### 特点

- CAS 根据局部对比度调整应用于图像不同区域的锐化量
- 有助于避免过度锐化高对比度区域和锐化不足低对比度区域
- 默认锐化强度为 0.6

### 使用示例

```rust
commands.spawn((
    Camera3dBundle::default(),
    Fxaa::default(),
    ContrastAdaptiveSharpening {
        enabled: true,
        sharpening_strength: 0.6,
        denoise: false,
    },
));
```

## 7. DLSS (Deep Learning Super Sampling)

### 核心概念

DLSS 是 NVIDIA 的深度学习超级采样技术，使用机器学习模型来 upscale 和抗锯齿图像。

### 特点

**优点**:
- 可以在较低分辨率下渲染，然后 upscale 到高分辨率，显著提高性能
- 质量通常优于传统的 upscaling 方法
- 支持光线重建 (Ray Reconstruction)

**缺点**:
- 仅支持 NVIDIA RTX GPU
- 需要 Windows/Linux Vulkan 渲染后端
- 需要项目 ID
- 有许可要求

### 使用步骤

1. 启用 Bevy 的 `dlss` 功能
2. 在应用设置期间，在 `DefaultPlugins` 之前插入 `DlssProjectId` 资源
3. 在运行时检查 `Option<Res<DlssSuperResolutionSupported>>` 以查看当前机器是否支持 DLSS
4. 将 `Dlss` 组件添加到相机实体，可选设置特定的 `DlssPerfQualityMode`
5. 可选地通过 `ContrastAdaptiveSharpening` 添加锐化

### 使用示例

```rust
// 1. 插入项目 ID
app.insert_resource(DlssProjectId(Uuid::parse_str("your-project-id").unwrap()));

// 2. 添加插件
app.add_plugins((
    DlssInitPlugin,
    DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            resolution: (1920, 1080).into(),
            ..default()
        }),
        ..default()
    }),
    DlssPlugin,
));

// 3. 检查支持
fn setup(
    mut commands: Commands,
    dlss_supported: Option<Res<DlssSuperResolutionSupported>>,
) {
    if dlss_supported.is_some() {
        commands.spawn((
            Camera3dBundle::default(),
            Dlss::super_resolution(DlssPerfQualityMode::Balanced),
            ContrastAdaptiveSharpening::default(),
        ));
    } else {
        commands.spawn(Camera3dBundle::default());
    }
}
```

## 8. 抗锯齿技术比较

| 技术 | 质量 | 性能 | 内存 | 适用场景 |
|------|------|------|------|----------|
| FXAA | 中等 | 优秀 | 低 | 需要快速抗锯齿的场景 |
| SMAA | 高 | 良好 | 中 | 需要高质量抗锯齿的场景 |
| TAA | 非常高 | 中等 | 高 | 3D 游戏，需要高质量且能接受 temporal artifacts 的场景 |
| DLSS | 非常高 | 优秀（upscale） | 高 | NVIDIA RTX GPU，需要性能提升的场景 |

## 9. 与其他模块的集成

### bevy_core_pipeline

- 使用 `Core2d` 和 `Core3d` 调度
- 使用 `tonemapping` 系统
- 使用 `FullscreenShader`

### bevy_render

- 使用 `ExtractComponent` 和 `ExtractComponentPlugin`
- 使用 `RenderDevice`、`RenderQueue`、`RenderPipeline` 等
- 使用 `ViewTarget`、`ExtractedView` 等

### bevy_camera

- 与 `Camera` 组件一起使用
- 与 `Msaa` 组件交互

### bevy_post_process

- 与 bloom、motion blur 等后处理效果集成

## 10. 最佳实践

### 1. 选择合适的抗锯齿技术

- **2D 游戏**: FXAA 或 SMAA
- **3D 游戏**: TAA 或 DLSS（如果有 NVIDIA GPU）
- **性能优先**: FXAA
- **质量优先**: TAA 或 DLSS

### 2. 结合使用 CAS

在使用 FXAA 或 TAA 时，结合使用 CAS 来恢复细节：

```rust
commands.spawn((
    Camera3dBundle::default(),
    TemporalAntiAliasing::default(),
    ContrastAdaptiveSharpening {
        enabled: true,
        sharpening_strength: 0.6,
        denoise: false,
    },
));
```

### 3. 正确配置 MSAA

- TAA 和 SMAA 通常需要禁用 MSAA
- FXAA 可以与 MSAA 一起使用，但通常不需要

### 4. 处理运动向量

- TAA 和 DLSS 需要正确的运动向量
- 确保所有渲染的对象都写入运动向量
- 粒子效果等特殊效果需要正确处理

### 5. 测试不同设置

- 尝试不同的灵敏度/预设值
- 在目标硬件上测试性能
- 观察 artifacts 并调整参数

## 11. 总结

`bevy_anti_alias` 提供了多种抗锯齿技术，每种都有其优缺点和适用场景：

- **FXAA**: 快速但质量中等
- **SMAA**: 高质量但性能开销较大
- **TAA**: 非常高质量但有 temporal artifacts
- **DLSS**: 非常高质量且性能优秀，但仅支持 NVIDIA GPU
- **CAS**: 用于恢复抗锯齿导致的细节损失

选择合适的抗锯齿技术取决于你的具体需求：性能目标、质量要求、目标硬件等。通常建议在 3D 游戏中使用 TAA 或 DLSS，在 2D 游戏中使用 FXAA 或 SMAA，并结合使用 CAS 来恢复细节。
