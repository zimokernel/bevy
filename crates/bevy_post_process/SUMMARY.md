# bevy_post_process 模块总结

## 概述

`bevy_post_process` 是 Bevy 游戏引擎的**后处理效果核心模块**，提供了完整的后处理效果系统，包括 Bloom、运动模糊、景深、自动曝光和效果堆栈。它是实现高级视觉效果的关键组件。

**核心特性**：
- **多种后处理效果**：Bloom、运动模糊、景深、自动曝光
- **模块化设计**：每个效果独立实现，易于扩展
- **高性能**：基于 GPU 的并行处理
- **可配置性**：每个效果都有详细的参数配置
- **HDR 支持**：完整的高动态范围渲染支持

---

## 核心架构

```
Post Processing System（后处理系统）
├── Effect Stack（效果堆栈）
│   ├── Bloom（ bloom 效果）
│   ├── Motion Blur（运动模糊）
│   ├── Depth of Field（景深）
│   ├── Auto Exposure（自动曝光）
│   └── Chromatic Aberration（色差）
├── Pipeline System（管线系统）
│   ├── Downsampling（下采样）
│   ├── Upsampling（上采样）
│   └── Composite（合成）
├── Render Graph（渲染图）
│   ├── Node（节点）
│   ├── Edge（边）
│   └── Pass（渲染通道）
└── Material System（材质系统）
    ├── Shader（着色器）
    ├── Uniform（ uniform 缓冲）
    └── Bind Group（绑定组）
```

**关键设计**：
- **模块化**：每个效果独立为插件
- **可组合**：效果可以按顺序组合
- **性能优化**：使用 mipmap 金字塔、并行处理
- **HDR 支持**：完整的高动态范围处理

---

## 主要子模块

### 模块功能总览

| 模块 | 功能 | 关键组件 |
|------|------|----------|
| **bloom/** | 光晕效果 | `Bloom`, `BloomDownsamplingPipeline`, `BloomUpsamplingPipeline` |
| **motion_blur/** | 运动模糊 | `MotionBlur`, `MotionBlurPipeline`, `MotionBlurUniform` |
| **auto_exposure/** | 自动曝光 | `AutoExposure`, `AutoExposurePipeline` |
| **dof/** | 景深 | `DepthOfField`, `DepthOfFieldPipeline` |
| **effect_stack/** | 效果堆栈 | `EffectStack`, `ChromaticAberration` |
| **msaa_writeback.rs** | MSAA 回写 | `MsaaWritebackPlugin` |

---

## 核心子模块详解

### 1. Bloom 效果

**文件**: [`bloom/mod.rs`](file:///d:/work/ttc/bevy/crates/bevy_post_process/src/bloom/mod.rs)

#### Bloom 结构定义

```rust
#[derive(Component, Reflect, Clone)]
pub struct Bloom {
    /// 整体强度（默认：0.15）
    pub intensity: f32,
    
    /// 低频增强（默认：0.7）
    pub low_frequency_boost: f32,
    
    /// 低频增强曲率（默认：0.95）
    pub low_frequency_boost_curvature: f32,
    
    /// 高通频率（默认：1.0）
    pub high_pass_frequency: f32,
    
    /// 预过滤设置
    pub prefilter: BloomPrefilter,
    
    /// 合成模式
    pub composite_mode: BloomCompositeMode,
    
    /// 最大 mip 维度（默认：512）
    pub max_mip_dimension: u32,
    
    /// 缩放因子（默认：Vec2::ONE）
    pub scale: Vec2,
}
```

**预设配置**：

```rust
impl Bloom {
    /// 自然预设（能量守恒）
    pub const NATURAL: Self = Self {
        intensity: 0.15,
        low_frequency_boost: 0.7,
        low_frequency_boost_curvature: 0.95,
        high_pass_frequency: 1.0,
        prefilter: BloomPrefilter { threshold: 0.0, threshold_softness: 0.0 },
        composite_mode: BloomCompositeMode::EnergyConserving,
        max_mip_dimension: 512,
        scale: Vec2::ONE,
    };
    
    /// 变形镜头预设（水平拉伸）
    pub const ANAMORPHIC: Self = Self {
        max_mip_dimension: 1024,
        scale: Vec2::new(4.0, 1.0),
        ..Self::NATURAL
    };
    
    /// 老派预设（类似旧游戏）
    pub const OLD_SCHOOL: Self = Self {
        intensity: 0.05,
        prefilter: BloomPrefilter { threshold: 0.6, threshold_softness: 0.2 },
        composite_mode: BloomCompositeMode::Additive,
        ..Self::NATURAL
    };
    
    /// 屏幕模糊预设（强模糊）
    pub const SCREEN_BLUR: Self = Self {
        intensity: 1.0,
        low_frequency_boost: 0.0,
        low_frequency_boost_curvature: 0.0,
        high_pass_frequency: 1.0 / 3.0,
        prefilter: BloomPrefilter { threshold: 0.0, threshold_softness: 0.0 },
        ..Self::NATURAL
    };
}
```

**参数说明**：

| 参数 | 默认值 | 范围 | 说明 |
|------|--------|------|------|
| `intensity` | 0.15 | 0-1 | 整体 bloom 强度 |
| `low_frequency_boost` | 0.7 | 0-1 | 低频（大模糊）增强 |
| `low_frequency_boost_curvature` | 0.95 | 0-1 | 增强曲线的曲率 |
| `high_pass_frequency` | 1.0 | 0.33-1 | 高通滤波器频率 |
| `max_mip_dimension` | 512 | 64-2048 | 最大 mip 级别大小 |
| `scale` | Vec2::ONE | 任意 | 各向异性缩放 |

#### Bloom 实现原理

**Mipmap 金字塔**：

```text
原始纹理（1920x1080）
    ↓ 下采样
Mip 1（960x540）
    ↓ 下采样
Mip 2（480x270）
    ↓ 下采样
Mip 3（240x135）
    ↓ 下采样
Mip 4（120x67）
    ↓ 下采样
Mip 5（60x33）
    ↓ 上采样 + 混合
Mip 4（120x67）
    ↓ 上采样 + 混合
Mip 3（240x135）
    ↓ 上采样 + 混合
Mip 2（480x270）
    ↓ 上采样 + 混合
Mip 1（960x540）
    ↓ 上采样 + 混合
最终结果（1920x1080）
```

**关键技术**：

1. **Firefly 消除**：
   - 首次下采样使用加权平均
   - 限制亮度范围在 [0, 1]
   - 参考：Brian Karis 方法

2. **参数化混合**：
   ```text
   混合因子 = parametric_curve(mip_level, curvature)
   结果 += 混合因子 * mip_color
   ```

3. **能量守恒**：
   - 确保总能量不变
   - 避免过亮或过暗

#### Bloom 管线

**下采样管线**：

```rust
#[derive(PartialEq, Eq, Hash, Clone)]
pub struct BloomDownsamplingPipelineKeys {
    prefilter: bool,           // 是否预过滤
    first_downsample: bool,    // 是否首次下采样
    uniform_scale: bool,       // 是否均匀缩放
}

impl SpecializedRenderPipeline for BloomDownsamplingPipeline {
    type Key = BloomDownsamplingPipelineKeys;
    
    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        // 根据 key 配置不同的管线
        // 首次下采样：Firefly 消除
        // 预过滤：阈值处理
        // 均匀缩放：各向同性/各向异性
    }
}
```

**上采样管线**：

```rust
#[derive(PartialEq, Eq, Hash, Clone)]
pub struct BloomUpsamplingPipelineKeys {
    composite_mode: BloomCompositeMode,  // 合成模式
    final_pipeline: bool,                 // 是否最终管线
}

impl SpecializedRenderPipeline for BloomUpsamplingPipeline {
    type Key = BloomUpsamplingPipelineKeys;
    
    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        // 能量守恒模式：使用 blend 常量
        // 加法模式：简单相加
        // 最终管线：输出到 HDR 纹理
    }
}
```

**合成模式**：

```rust
#[derive(Debug, Clone, Reflect, PartialEq, Eq, Hash, Copy)]
pub enum BloomCompositeMode {
    /// 能量守恒模式（推荐）
    EnergyConserving,
    
    /// 加法模式（简单）
    Additive,
}
```

**着色器实现**：

```wgsl
// bloom.wgsl

// 下采样：9 样本加权平均
fn downsample(uv: vec2<f32>) -> vec3<f32> {
    let a = textureSample(input_texture, s, uv + vec2<f32>(-2, 2) * ps);
    let b = textureSample(input_texture, s, uv + vec2<f32>(0, 2) * ps);
    let c = textureSample(input_texture, s, uv + vec2<f32>(2, 2) * ps);
    // ... 其他样本
    
    // 加权平均
    return (a*0.5 + b*1.0 + c*0.5 + ...) / weight_sum;
}

// 上采样：双线性插值
fn upsample(uv: vec2<f32>) -> vec3<f32> {
    return textureSample(input_texture, s, uv);
}
```

**使用示例**：

```rust
use bevy::prelude::*;
use bevy_post_process::bloom::Bloom;

fn setup(mut commands: Commands) {
    commands.spawn((
        Camera3dBundle {
            camera: Camera {
                hdr: true,  // 必须启用 HDR
                ..default()
            },
            ..default()
        },
        // 使用预设
        Bloom::NATURAL,
        // 或自定义
        Bloom {
            intensity: 0.2,
            low_frequency_boost: 0.8,
            ..Bloom::NATURAL
        },
    ));
}
```

---

### 2. 运动模糊效果

**文件**: [`motion_blur/mod.rs`](file:///d:/work/ttc/bevy/crates/bevy_post_process/src/motion_blur/mod.rs)

#### MotionBlur 结构定义

```rust
#[derive(Reflect, Component, Clone)]
pub struct MotionBlur {
    /// 快门角度（默认：0.5）
    /// 0.5 = 180 度（电影标准）
    /// 1.0 = 360 度（完全模糊）
    pub shutter_angle: f32,
    
    /// 采样数量（默认：1）
    /// 1 = 3 样本（-1, 0, 1）
    /// 3 = 7 样本（-3, -2, -1, 0, 1, 2, 3）
    pub samples: u32,
}

impl Default for MotionBlur {
    fn default() -> Self {
        Self {
            shutter_angle: 0.5,
            samples: 1,
        }
    }
}
```

**参数说明**：

| 参数 | 默认值 | 范围 | 说明 |
|------|--------|------|------|
| `shutter_angle` | 0.5 | 0-2 | 快门打开时间比例 |
| `samples` | 1 | 0-8 | 每个方向的采样数 |

**快门角度**：

```text
0.0 = 无模糊
0.5 = 180 度（电影标准）
1.0 = 360 度（完全模糊）
>1.0 = 艺术效果（非物理）
```

#### 运动模糊实现原理

**运动向量**：

```text
运动向量 = 当前位置 - 上一帧位置
         = (current_uv - previous_uv)

在着色器中：
motion_vector = textureSample(motion_vectors_texture, uv).rg
```

**曝光向量**：

```text
曝光向量 = 快门角度 × 运动向量
        = shutter_angle × motion_vector

表示：快门打开期间，像素移动的距离
```

**多采样模糊**：

```text
对于 samples = 2：
样本位置 = uv + exposure_vector × (-2 + noise) / 2
样本位置 = uv + exposure_vector × (-1 + noise) / 2
样本位置 = uv + exposure_vector × (0 + noise) / 2
样本位置 = uv + exposure_vector × (1 + noise) / 2

noise = 交错梯度噪声（打破周期性）
```

**深度测试**：

```text
如果样本深度 < 当前像素深度：
    样本在当前像素前面
    不应该影响当前像素
    权重 = 0

如果样本深度 > 当前像素深度：
    样本在当前像素后面
    应该影响当前像素
    权重 = 1
```

**运动一致性测试**：

```text
如果样本运动方向与当前像素运动方向差异大：
    样本可能不属于同一物体
    权重降低

cos_angle = dot(step_vector, sample_motion)
motion_similarity = clamp(abs(cos_angle), 0, 1)
```

#### 运动模糊管线

**绑定组布局**：

```rust
pub struct MotionBlurPipeline {
    pub sampler: Sampler,
    pub layout: BindGroupLayoutDescriptor,        // 非 MSAA
    pub layout_msaa: BindGroupLayoutDescriptor,   // MSAA
    pub fullscreen_shader: FullscreenShader,
    pub fragment_shader: Handle<Shader>,
}

// 绑定组条目：
// 0: 屏幕纹理
// 1: 运动向量纹理
// 2: 深度纹理
// 3: 采样器
// 4: 运动模糊设置
// 5: 全局 uniforms
```

**着色器实现**：

```wgsl
// motion_blur.wgsl

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    // 1. 获取基础颜色
    let base_color = textureSample(screen_texture, sampler, in.uv);
    
    // 2. 获取运动向量
    let motion_vector = textureSample(motion_vectors, sampler, in.uv).rg;
    
    // 3. 计算曝光向量
    let exposure_vector = settings.shutter_angle * motion_vector;
    
    // 4. 多采样模糊
    var accumulator = vec4<f32>(0.0);
    var weight_total = 0.0;
    let noise = interleaved_gradient_noise(frag_coords, frame_count);
    
    for (var i = -n_samples; i < n_samples; i++) {
        let step_vector = 0.5 * exposure_vector * (f32(i) + noise) / f32(n_samples);
        let sample_uv = in.uv + step_vector;
        
        // 边界检查
        if sample_uv.x < 0.0 || sample_uv.x > 1.0 || 
           sample_uv.y < 0.0 || sample_uv.y > 1.0 {
            continue;
        }
        
        // 深度测试
        let sample_depth = textureSample(depth_texture, sampler, sample_uv);
        let current_depth = textureSample(depth_texture, sampler, in.uv);
        
        var weight = 1.0;
        if sample_depth < current_depth && sample_depth > 0.0 {
            weight = 0.0; // 样本在前面，忽略
        }
        
        // 运动一致性测试
        // ...
        
        accumulator += weight * textureSample(screen_texture, sampler, sample_uv);
        weight_total += weight;
    }
    
    // 5. 返回结果
    if weight_total <= 0.0 || has_moved_less_than_a_pixel {
        return base_color;
    }
    return accumulator / weight_total;
}
```

**使用示例**：

```rust
use bevy::prelude::*;
use bevy_post_process::motion_blur::MotionBlur;
use bevy_core_pipeline::prepass::{DepthPrepass, MotionVectorPrepass};

fn setup(mut commands: Commands) {
    commands.spawn((
        Camera3dBundle {
            camera: Camera {
                hdr: true,
                ..default()
            },
            ..default()
        },
        // 必须启用预传递
        DepthPrepass,
        MotionVectorPrepass,
        // 运动模糊设置
        MotionBlur {
            shutter_angle: 0.5,  // 180 度
            samples: 2,          // 5 样本
        },
    ));
}
```

---

### 3. 自动曝光效果

**文件**: [`auto_exposure/mod.rs`](file:///d:/work/ttc/bevy/crates/bevy_post_process/src/auto_exposure/mod.rs)

#### AutoExposure 结构定义

```rust
#[derive(Component, Reflect, Clone)]
pub struct AutoExposure {
    /// 最小 EV100 值（默认：3.0）
    pub min_ev100: f32,
    
    /// 最大 EV100 值（默认：16.0）
    pub max_ev100: f32,
    
    /// 适应速度（默认：0.1）
    pub adaptation_speed: f32,
    
    /// 直方图区域（默认：0.1-0.9）
    pub histogram_region: (f32, f32),
    
    /// 目标亮度（默认：0.18）
    pub target_luminance: f32,
}
```

**参数说明**：

| 参数 | 默认值 | 范围 | 说明 |
|------|--------|------|------|
| `min_ev100` | 3.0 | 1-10 | 最暗曝光 |
| `max_ev100` | 16.0 | 10-20 | 最亮曝光 |
| `adaptation_speed` | 0.1 | 0.01-1 | 适应速度（越大越快）| 
| `histogram_region` | (0.1, 0.9) | (0,1) | 直方图分析范围 |
| `target_luminance` | 0.18 | 0.01-1 | 目标亮度（中灰色）| 

#### 自动曝光实现原理

**亮度直方图**：

```text
1. 将屏幕划分为 256x256 网格
2. 对每个网格计算平均亮度
3. 构建亮度直方图（256 个 bin）
4. 分析直方图统计信息
```

**关键步骤**：

```text
1. 下采样到低分辨率（64x64）
2. 计算每个 tile 的平均亮度
3. 构建直方图
4. 找到百分位亮度（如 90%）
5. 计算目标 EV100
6. 平滑过渡到目标 EV100
```

**EV100 计算**：

```text
当前亮度 = 场景平均亮度
目标亮度 = 0.18（中灰色）

EV100 变化 = log2(当前亮度 / 目标亮度)

新 EV100 = clamp(
    旧 EV100 + adaptation_speed × EV100 变化,
    min_ev100,
    max_ev100
)
```

**补偿曲线**：

```text
为了避免曝光突变，使用补偿曲线：

如果当前亮度 < 目标亮度：
    曝光增加（EV100 减小）
    曲线更陡（快速适应）

如果当前亮度 > 目标亮度：
    曝光减少（EV100 增加）
    曲线更缓（缓慢适应）
```

---

### 4. 景深效果

**文件**: [`dof/mod.rs`](file:///d:/work/ttc/bevy/crates/bevy_post_process/src/dof/mod.rs)

#### DepthOfField 结构定义

```rust
#[derive(Component, Reflect, Clone)]
pub struct DepthOfField {
    /// 焦点距离（默认：10.0）
    pub focus_distance: f32,
    
    /// 光圈大小（默认：0.1）
    pub aperture_size: f32,
    
    /// 焦距（默认：50.0mm）
    pub focal_length: f32,
    
    /// 传感器尺寸（默认：36.0mm）
    pub sensor_size: f32,
    
    /// 散景形状（默认：圆形）
    pub bokeh_shape: BokehShape,
    
    /// 散景旋转（默认：0.0）
    pub bokeh_rotation: f32,
}
```

**参数说明**：

| 参数 | 默认值 | 范围 | 说明 |
|------|--------|------|------|
| `focus_distance` | 10.0 | 0.1-1000 | 焦点距离（米）| 
| `aperture_size` | 0.1 | 0.01-1 | 光圈大小（f-stop 倒数）| 
| `focal_length` | 50.0 | 10-200 | 焦距（毫米）| 
| `sensor_size` | 36.0 | 24-50 | 传感器尺寸（毫米）| 

#### 景深实现原理

**圆弥散斑（CoC）**：

```text
CoC = 模糊圈直径

CoC = (aperture_size × |depth - focus_distance|) / 
      (depth × (1 - focus_distance / depth))

简化：
CoC ∝ aperture_size
CoC ∝ |depth - focus_distance|
```

**双 pass 方法**：

```text
Pass 1: 前景模糊（深度 < 焦点距离）
Pass 2: 背景模糊（深度 > 焦点距离）
Pass 3: 合成（前景 + 背景 + 清晰）

或使用 mipmap 金字塔：
- 不同 CoC 使用不同 mip 级别
- 大 CoC 使用小 mip（更模糊）
- 小 CoC 使用大 mip（更清晰）
```

**散景效果**：

```text
圆形散景：
    使用高斯核
    自然、柔和

六边形散景：
    使用六边形核
    模拟真实光圈
    更艺术化

自定义形状：
    使用查找表（LUT）
    完全自定义
```

---

### 5. 效果堆栈

**文件**: [`effect_stack/mod.rs`](file:///d:/work/ttc/bevy/crates/bevy_post_process/src/effect_stack/mod.rs)

#### EffectStack 结构定义

```rust
#[derive(Component, Reflect, Clone)]
pub struct EffectStack {
    /// 色差强度（默认：0.0）
    pub chromatic_aberration: f32,
    
    /// 胶片颗粒强度（默认：0.0）
    pub film_grain: f32,
    
    /// 锐化强度（默认：0.0）
    pub sharpening: f32,
    
    ///  vignette 强度（默认：0.0）
    pub vignette: f32,
    
    /// 对比度（默认：1.0）
    pub contrast: f32,
    
    /// 饱和度（默认：1.0）
    pub saturation: f32,
    
    /// 亮度（默认：1.0）
    pub brightness: f32,
}
```

**参数说明**：

| 参数 | 默认值 | 范围 | 说明 |
|------|--------|------|------|
| `chromatic_aberration` | 0.0 | 0-0.1 | 色差强度 |
| `film_grain` | 0.0 | 0-1 | 胶片颗粒强度 |
| `sharpening` | 0.0 | 0-1 | 锐化强度 |
| `vignette` | 0.0 | 0-1 | 暗角强度 |
| `contrast` | 1.0 | 0.5-2 | 对比度 |
| `saturation` | 1.0 | 0-2 | 饱和度 |
| `brightness` | 1.0 | 0-2 | 亮度 |

#### 色差实现

```wgsl
// chromatic_aberration.wgsl

fn apply_chromatic_aberration(uv: vec2<f32>, strength: f32) -> vec3<f32> {
    // 计算中心距离
    let distance = length(uv - 0.5);
    
    // 计算偏移量（边缘更大）
    let offset = strength * distance * 0.01;
    
    // 红色通道（向左偏移）
    let r = textureSample(texture, sampler, uv - vec2<f32>(offset, 0)).r;
    
    // 绿色通道（不偏移）
    let g = textureSample(texture, sampler, uv).g;
    
    // 蓝色通道（向右偏移）
    let b = textureSample(texture, sampler, uv + vec2<f32>(offset, 0)).b;
    
    return vec3<f32>(r, g, b);
}
```

**胶片颗粒实现**：

```wgsl
fn apply_film_grain(uv: vec2<f32>, strength: f32, time: f32) -> vec3<f32> {
    // 生成噪声
    let noise = perlin_noise(uv * 100.0, time) * 0.5 + 0.5;
    
    // 应用到颜色
    let grain = 1.0 + (noise - 0.5) * strength;
    
    return color * grain;
}
```

**锐化实现**：

```wgsl
fn apply_sharpening(uv: vec2<f32>, strength: f32) -> vec3<f32> {
    // 拉普拉斯核
    let kernel = [
        0.0, -1.0, 0.0,
        -1.0, 5.0, -1.0,
        0.0, -1.0, 0.0
    ];
    
    // 卷积
    var result = vec3<f32>(0.0);
    for (var i = -1; i <= 1; i++) {
        for (var j = -1; j <= 1; j++) {
            result += textureSample(texture, sampler, uv + vec2<f32>(i, j) * ps) * 
                      kernel[(i+1)*3 + (j+1)];
        }
    }
    
    // 混合原始和锐化
    return mix(original, result, strength);
}
```

---

## 典型使用示例

### 1. 完整后处理设置

```rust
use bevy::prelude::*;
use bevy_post_process::prelude::*;
use bevy_core_pipeline::prepass::{DepthPrepass, MotionVectorPrepass};

fn setup_post_processing(mut commands: Commands) {
    commands.spawn((
        Camera3dBundle {
            camera: Camera {
                hdr: true,  // 必须启用 HDR
                ..default()
            },
            ..default()
        },
        // 启用预传递（用于运动模糊）
        DepthPrepass,
        MotionVectorPrepass,
        
        // Bloom 效果
        Bloom {
            intensity: 0.2,
            low_frequency_boost: 0.8,
            ..Bloom::NATURAL
        },
        
        // 运动模糊
        MotionBlur {
            shutter_angle: 0.5,  // 180 度
            samples: 2,          // 5 样本
        },
        
        // 自动曝光
        AutoExposure {
            adaptation_speed: 0.15,
            ..default()
        },
        
        // 效果堆栈
        EffectStack {
            chromatic_aberration: 0.02,
            film_grain: 0.1,
            sharpening: 0.2,
            vignette: 0.1,
            ..default()
        },
    ));
}
```

### 2. 电影风格设置

```rust
fn setup_cinema_style(mut commands: Commands) {
    commands.spawn((
        Camera3dBundle {
            camera: Camera { hdr: true, ..default() },
            transform: Transform::from_xyz(0.0, 2.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        },
        DepthPrepass,
        MotionVectorPrepass,
        
        // 变形镜头 Bloom
        Bloom::ANAMORPHIC,
        
        // 电影运动模糊（180 度）
        MotionBlur {
            shutter_angle: 0.5,
            samples: 3,  // 7 样本（高质量）
        },
        
        // 自动曝光（快速适应）
        AutoExposure {
            adaptation_speed: 0.2,
            target_luminance: 0.12,  // 稍暗（电影风格）
            ..default()
        },
        
        // 电影效果
        EffectStack {
            chromatic_aberration: 0.01,
            film_grain: 0.15,
            vignette: 0.2,
            contrast: 1.1,
            saturation: 0.9,
            ..default()
        },
    ));
}
```

### 3. 游戏风格设置

```rust
fn setup_game_style(mut commands: Commands) {
    commands.spawn((
        Camera3dBundle {
            camera: Camera { hdr: true, ..default() },
            ..default()
        },
        DepthPrepass,
        MotionVectorPrepass,
        
        // 强 Bloom（游戏风格）
        Bloom {
            intensity: 0.3,
            low_frequency_boost: 0.9,
            ..Bloom::NATURAL
        },
        
        // 运动模糊（游戏风格）
        MotionBlur {
            shutter_angle: 0.8,  // 强模糊
            samples: 1,          // 3 样本（性能优先）
        },
        
        // 自动曝光（游戏风格）
        AutoExposure {
            adaptation_speed: 0.3,  // 快速适应
            min_ev100: 5.0,
            max_ev100: 14.0,
            ..default()
        },
        
        // 游戏效果
        EffectStack {
            sharpening: 0.3,
            contrast: 1.2,
            saturation: 1.1,
            ..default()
        },
    ));
}
```

---

## 后处理流程

### 渲染顺序

```text
标准后处理顺序：

1. 几何渲染
   - 渲染所有 3D/2D 实体
   - 输出到 HDR 纹理

2. 预传递
   - 深度预传递
   - 运动向量预传递
   - 法线预传递

3. 自动曝光
   - 计算场景亮度
   - 调整曝光值

4. Bloom
   - 下采样到 mipmap 金字塔
   - 上采样并混合
   - 合成到原始图像

5. 运动模糊
   - 使用运动向量
   - 多采样模糊
   - 深度测试

6. 景深
   - 计算 CoC
   - 前景模糊
   - 背景模糊
   - 合成

7. 效果堆栈
   - 色差
   - 胶片颗粒
   - 锐化
   - 暗角
   - 对比度/饱和度/亮度

8. 色调映射
   - HDR → LDR
   - 应用色调映射曲线

9. Gamma 校正
   - 应用 Gamma 曲线
   - 输出到屏幕
```

### 关键优化

**1. Mipmap 金字塔**：
```text
Bloom 使用 mipmap 金字塔：
- 每个 mip 级别是前一级别的 1/2 大小
- 大模糊使用小 mip（性能高）
- 小模糊使用大 mip（质量高）
```

**2. 并行处理**：
```text
每个像素独立计算：
- GPU 可以并行处理所有像素
- 利用 GPU 的并行性
```

**3. 交错梯度噪声**：
```text
打破周期性图案：
- 使用噪声样本位置
- 避免 Moiré 图案
- 提高视觉质量
```

**4. 早期退出**：
```text
如果效果强度为 0：
- 跳过整个效果
- 节省性能
```

---

## 性能优化建议

### 1. 降低样本数量

```rust
// 运动模糊：降低样本数
MotionBlur {
    samples: 1,  // 3 样本（默认 1 = 3 样本）
    ..default()
}

// Bloom：降低最大 mip 维度
Bloom {
    max_mip_dimension: 256,  // 默认 512
    ..default()
}
```

### 2. 禁用不需要的效果

```rust
// 只启用需要的效果
commands.spawn((
    Camera3dBundle { ..default() },
    Bloom::NATURAL,           // 启用
    // MotionBlur::default(),  // 禁用
    // AutoExposure::default(), // 禁用
));
```

### 3. 降低分辨率

```rust
// 使用较低分辨率进行后处理
// 然后上采样到全屏
// 节省 75% 性能（1/2 分辨率）
```

### 4. 质量级别设置

```rust
// 低质量（性能优先）
if quality_level == QualityLevel::Low {
    commands.spawn((
        Bloom { max_mip_dimension: 128, ..default() },
        MotionBlur { samples: 0, ..default() },  // 禁用
    ));
}

// 高质量（质量优先）
if quality_level == QualityLevel::High {
    commands.spawn((
        Bloom { max_mip_dimension: 1024, ..default() },
        MotionBlur { samples: 3, ..default() },  // 7 样本
    ));
}
```

---

## 文件结构

```
src/
├── bloom/                     # Bloom 效果
│   ├── mod.rs
│   ├── settings.rs           # Bloom 设置
│   ├── downsampling_pipeline.rs
│   ├── upsampling_pipeline.rs
│   └── bloom.wgsl            # Bloom 着色器
├── motion_blur/              # 运动模糊
│   ├── mod.rs
│   ├── pipeline.rs
│   ├── node.rs
│   └── motion_blur.wgsl      # 运动模糊着色器
├── auto_exposure/            # 自动曝光
│   ├── mod.rs
│   ├── settings.rs
│   ├── pipeline.rs
│   ├── node.rs
│   ├── buffers.rs
│   ├── compensation_curve.rs
│   └── auto_exposure.wgsl
├── dof/                      # 景深
│   ├── mod.rs
│   └── dof.wgsl
├── effect_stack/             # 效果堆栈
│   ├── mod.rs
│   ├── post_process.wgsl
│   └── chromatic_aberration.wgsl
├── msaa_writeback.rs         # MSAA 回写
├── gaussian_blur.wgsl        # 高斯模糊着色器
└── lib.rs                    # 主入口和 PostProcessPlugin
```

---

## 常见问题

### 1. Bloom 不工作

**可能原因**：
- 未启用 HDR
- 场景没有足够亮的物体
- Bloom 强度设置为 0

**解决方法**：
```rust
Camera3dBundle {
    camera: Camera {
        hdr: true,  // 必须启用
        ..default()
    },
    ..default()
}

// 使用 emissive 材质
StandardMaterial {
    emissive: Color::rgb(10.0, 10.0, 10.0),  // 足够亮
    ..default()
}
```

### 2. 运动模糊不工作

**可能原因**：
- 未启用 MotionVectorPrepass
- 物体没有移动
- 快门角度设置为 0

**解决方法**：
```rust
commands.spawn((
    Camera3dBundle { ..default() },
    DepthPrepass,           // 必须
    MotionVectorPrepass,    // 必须
    MotionBlur {
        shutter_angle: 0.5,  // 不能为 0
        ..default()
    },
));
```

### 3. 性能问题

**可能原因**：
- 样本数量过多
- 分辨率过高
- 启用了太多效果

**解决方法**：
```rust
// 降低样本数
MotionBlur { samples: 1, ..default() }

// 降低 Bloom 分辨率
Bloom { max_mip_dimension: 256, ..default() }

// 禁用不需要的效果
// AutoExposure::default(), // 禁用
```

---

## 总结

`bevy_post_process` 是一个**功能完整、性能优化的后处理系统**，具有以下优势：

**核心优势**：
1. **多种效果**：Bloom、运动模糊、景深、自动曝光、效果堆栈
2. **高质量**：基于物理的实现，视觉效果出色
3. **高性能**：使用 mipmap 金字塔、并行处理、早期退出
4. **可配置**：每个效果都有详细的参数配置
5. **可扩展**：模块化设计，易于添加新效果

**适用场景**：
- 3D 游戏（需要高级视觉效果）
- 电影渲染（需要电影级效果）
- 可视化（需要高质量渲染）
- VR/AR（需要沉浸式效果）

**学习资源**：
- [Bevy Post Process 文档](https://docs.rs/bevy/latest/bevy/post_process/index.html)
- [Real-time Rendering Book](https://www.realtimerendering.com/)
- [Call of Duty Post Processing](http://www.iryoku.com/next-generation-post-processing-in-call-of-duty-advanced-warfare)
- [Physically Based Bloom](https://learnopengl.com/Guest-Articles/2022/Phys.-Based-Bloom)

---

**注意**：`bevy_post_process` 是高级视觉效果模块，需要与 `bevy_core_pipeline`、`bevy_pbr` 和 `bevy_render` 紧密配合。合理使用后处理可以显著提升游戏的视觉质量，但也会增加性能开销。
