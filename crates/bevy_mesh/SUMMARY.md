# bevy_mesh 模块总结

## 概述

`bevy_mesh` 是 Bevy 游戏引擎的**网格数据核心模块**，提供了完整的 3D/2D 网格数据结构、顶点属性管理、图元生成和高级网格处理功能。它是 Bevy 渲染系统的基础，负责管理所有几何数据。

**核心特性**：
- **灵活的网格结构**：支持自定义顶点属性
- **丰富的图元库**：12 种 3D 图元和 2D 图元
- **高级网格处理**：法线计算、切线生成、UV 映射
- **蒙皮动画**：支持骨骼动画
- **变形目标**：支持 morph 动画
- **序列化**：支持网格序列化和反序列化

---

## 核心架构

### 网格数据模型

```
Mesh（网格）
├── PrimitiveTopology（图元拓扑）
├── Attributes（顶点属性）
│   ├── Position（位置）
│   ├── Normal（法线）
│   ├── UV（纹理坐标）
│   ├── Tangent（切线）
│   ├── Color（颜色）
│   ├── Joint Weight（关节权重）
│   └── Joint Index（关节索引）
├── Indices（索引）
├── Morph Targets（变形目标）
└── AABB（包围盒）
```

**关键设计**：
- **BTreeMap**：使用有序映射存储顶点属性，确保稳定的迭代顺序
- **Extractable Data**：支持将数据提取到渲染世界，避免重复处理
- **Asset System**：作为 Asset 管理，支持热重载和引用计数

---

## 主要子模块

### 模块功能总览

| 模块 | 功能 | 关键组件 |
|------|------|----------|
| **mesh.rs** | 核心 Mesh 结构 | `Mesh`, `MeshExtractableData`, `MeshAccessError` |
| **vertex.rs** | 顶点属性管理 | `MeshVertexAttribute`, `VertexAttributeValues` |
| **index.rs** | 索引管理 | `Indices`, `IndexFormat` |
| **primitives/** | 图元生成 | `Meshable`, `MeshBuilder`, 12+ 图元类型 |
| **skinning.rs** | 蒙皮动画 | `SkinnedMesh`, `SkinnedMeshInverseBindposes` |
| **morph.rs** | 变形动画 | `MorphWeights`, `MorphTarget` |
| **mikktspace.rs** | 切线生成 | `generate_tangents`, `MikkTSpace` |
| **components.rs** | ECS 组件 | `Mesh2d`, `Mesh3d` |

---

## 核心子模块详解

### 1. Mesh 结构

**文件**: [`mesh.rs`](file:///d:/work/ttc/bevy/crates/bevy_mesh/src/mesh.rs)

#### Mesh 定义

```rust
#[derive(Asset, Debug, Clone, Reflect, PartialEq)]
pub struct Mesh {
    // 图元拓扑（三角形列表、线条等）
    primitive_topology: PrimitiveTopology,
    
    // 顶点属性映射（位置、法线、UV 等）
    // 使用 BTreeMap 确保稳定的迭代顺序
    attributes: MeshExtractableData<BTreeMap<MeshVertexAttributeId, MeshAttributeData>>,
    
    // 索引数据（可选）
    indices: MeshExtractableData<Indices>,
    
    // 变形目标（可选，需要 morph feature）
    #[cfg(feature = "morph")]
    morph_targets: MeshExtractableData<Handle<Image>>,
    
    #[cfg(feature = "morph")]
    morph_target_names: MeshExtractableData<Vec<String>>,
    
    // Asset 使用方式
    pub asset_usage: RenderAssetUsages,
    
    // 是否启用光线追踪 BLAS 构建
    pub enable_raytracing: bool,
    
    // 预计算的 AABB
    pub final_aabb: Option<Aabb3d>,
}
```

#### 标准顶点属性

```rust
impl Mesh {
    // 顶点位置（必需）
    pub const ATTRIBUTE_POSITION: MeshVertexAttribute = 
        MeshVertexAttribute::new("Vertex_Position", 0, VertexFormat::Float32x3);
    
    // 法线（用于光照计算）
    pub const ATTRIBUTE_NORMAL: MeshVertexAttribute = 
        MeshVertexAttribute::new("Vertex_Normal", 1, VertexFormat::Float32x3);
    
    // UV 坐标（纹理映射）
    pub const ATTRIBUTE_UV_0: MeshVertexAttribute = 
        MeshVertexAttribute::new("Vertex_Uv", 2, VertexFormat::Float32x2);
    
    // 第二套 UV 坐标（用于光照贴图）
    pub const ATTRIBUTE_UV_1: MeshVertexAttribute = 
        MeshVertexAttribute::new("Vertex_Uv_1", 3, VertexFormat::Float32x2);
    
    // 切线（用于法线贴图）
    pub const ATTRIBUTE_TANGENT: MeshVertexAttribute = 
        MeshVertexAttribute::new("Vertex_Tangent", 4, VertexFormat::Float32x4);
    
    // 顶点颜色
    pub const ATTRIBUTE_COLOR: MeshVertexAttribute = 
        MeshVertexAttribute::new("Vertex_Color", 5, VertexFormat::Float32x4);
    
    // 关节权重（蒙皮动画）
    pub const ATTRIBUTE_JOINT_WEIGHT: MeshVertexAttribute = 
        MeshVertexAttribute::new("Vertex_JointWeight", 6, VertexFormat::Float32x4);
    
    // 关节索引（蒙皮动画）
    pub const ATTRIBUTE_JOINT_INDEX: MeshVertexAttribute = 
        MeshVertexAttribute::new("Vertex_JointIndex", 7, VertexFormat::Uint32x4);
}
```

#### MeshExtractableData 枚举

```rust
// 支持三种状态的数据存储
#[derive(Debug, Clone, PartialEq, Reflect, Default)]
enum MeshExtractableData<T> {
    Data(T),              // 有数据
    #[default]
    NoData,               // 无数据
    ExtractedToRenderWorld, // 已提取到渲染世界
}

impl<T> MeshExtractableData<T> {
    // 获取数据引用
    fn as_ref(&self) -> Result<&T, MeshAccessError> { ... }
    
    // 获取可变引用
    fn as_mut(&mut self) -> Result<&mut T, MeshAccessError> { ... }
    
    // 提取数据（移动语义）
    fn extract(&mut self) -> Result<MeshExtractableData<T>, MeshAccessError> { ... }
    
    // 替换数据
    fn replace(&mut self, data: impl Into<MeshExtractableData<T>>) -> Result<Option<T>, MeshAccessError> { ... }
}
```

**设计目的**：
- **提取保护**：防止在渲染世界中重复使用数据
- **错误处理**：明确的错误信息，帮助调试
- **移动语义**：支持高效的数据转移

#### MeshAccessError 枚举

```rust
#[derive(Error, Debug, Clone)]
pub enum MeshAccessError {
    #[error("The mesh vertex/index data has been extracted to the RenderWorld (via `Mesh::asset_usage`)")]
    ExtractedToRenderWorld,
    
    #[error("The requested mesh data wasn't found in this mesh")]
    NotFound,
}
```

---

### 2. 图元生成（Primitives）

**文件**: [`primitives/mod.rs`](file:///d:/work/ttc/bevy/crates/bevy_mesh/src/primitives/mod.rs)

#### Meshable 特质

```rust
/// 可转换为 Mesh 的形状特质
pub trait Meshable {
    type Output: MeshBuilder;
    
    /// 创建 Mesh 构建器
    fn mesh(&self) -> Self::Output;
}
```

#### MeshBuilder 特质

```rust
/// 用于构建 Mesh 的特质
pub trait MeshBuilder {
    /// 基于配置构建 Mesh
    fn build(&self) -> Mesh;
}

impl<T: MeshBuilder> From<T> for Mesh {
    fn from(builder: T) -> Self {
        builder.build()
    }
}
```

#### 3D 图元列表

| 图元 | 文件 | 特点 |
|------|------|------|
| **Capsule** | [`capsule.rs`](file:///d:/work/ttc/bevy/crates/bevy_mesh/src/primitives/dim3/capsule.rs) | 胶囊形状 |
| **Cone** | [`cone.rs`](file:///d:/work/ttc/bevy/crates/bevy_mesh/src/primitives/dim3/cone.rs) | 圆锥 |
| **ConicalFrustum** | [`conical_frustum.rs`](file:///d:/work/ttc/bevy/crates/bevy_mesh/src/primitives/dim3/conical_frustum.rs) | 圆台 |
| **Cuboid** | [`cuboid.rs`](file:///d:/work/ttc/bevy/crates/bevy_mesh/src/primitives/dim3/cuboid.rs) | 长方体 |
| **Cylinder** | [`cylinder.rs`](file:///d:/work/ttc/bevy/crates/bevy_mesh/src/primitives/dim3/cylinder.rs) | 圆柱 |
| **Plane** | [`plane.rs`](file:///d:/work/ttc/bevy/crates/bevy_mesh/src/primitives/dim3/plane.rs) | 平面 |
| **Polyline3d** | [`polyline3d.rs`](file:///d:/work/ttc/bevy/crates/bevy_mesh/src/primitives/dim3/polyline3d.rs) | 折线 |
| **Segment3d** | [`segment3d.rs`](file:///d:/work/ttc/bevy/crates/bevy_mesh/src/primitives/dim3/segment3d.rs) | 线段 |
| **Sphere** | [`sphere.rs`](file:///d:/work/ttc/bevy/crates/bevy_mesh/src/primitives/dim3/sphere.rs) | 球体 |
| **Tetrahedron** | [`tetrahedron.rs`](file:///d:/work/ttc/bevy/crates/bevy_mesh/src/primitives/dim3/tetrahedron.rs) | 四面体 |
| **Torus** | [`torus.rs`](file:///d:/work/ttc/bevy/crates/bevy_mesh/src/primitives/dim3/torus.rs) | 圆环 |
| **Triangle3d** | [`triangle3d.rs`](file:///d:/work/ttc/bevy/crates/bevy_mesh/src/primitives/dim3/triangle3d.rs) | 三角形 |

#### 图元构建器示例

```rust
// Cuboid 构建器
#[derive(Clone, Copy, Debug, Reflect)]
pub struct CuboidMeshBuilder {
    half_size: Vec3,
}

impl MeshBuilder for CuboidMeshBuilder {
    fn build(&self) -> Mesh {
        // 生成 8 个顶点
        let vertices = [
            Vec3::new(-half_size.x, -half_size.y, -half_size.z),
            Vec3::new(half_size.x, -half_size.y, -half_size.z),
            // ... 其他 6 个顶点
        ];
        
        // 生成 12 个三角形（每个面 2 个）
        let indices = Indices::U32(vec![
            0, 1, 2, 0, 2, 3,  // 前面
            4, 5, 6, 4, 6, 7,  // 后面
            // ... 其他面
        ]);
        
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vertices)
            .with_inserted_indices(indices)
    }
}

impl Meshable for Cuboid {
    type Output = CuboidMeshBuilder;
    
    fn mesh(&self) -> Self::Output {
        CuboidMeshBuilder {
            half_size: self.half_size,
        }
    }
}
```

**使用示例**：

```rust
use bevy::prelude::*;
use bevy_math::primitives::Cuboid;
use bevy_mesh::Meshable;

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    // 简单方式
    let cuboid_mesh = meshes.add(Cuboid::default());
    
    // 自定义大小
    let custom_cuboid = meshes.add(Cuboid {
        half_size: Vec3::new(1.0, 2.0, 1.0),
    });
    
    // 使用构建器
    let builder_mesh = meshes.add(
        Cuboid::default()
            .mesh()
            .build()
    );
}
```

---

### 3. 蒙皮动画（Skinning）

**文件**: [`skinning.rs`](file:///d:/work/ttc/bevy/crates/bevy_mesh/src/skinning.rs)

#### SkinnedMesh 结构

```rust
#[derive(Component, Debug, Default, Clone, Reflect)]
pub struct SkinnedMesh {
    /// 逆绑定姿势矩阵
    pub inverse_bindposes: Handle<SkinnedMeshInverseBindposes>,
    
    /// 关节实体列表
    #[entities]
    pub joints: Vec<Entity>,
}
```

#### SkinnedMeshInverseBindposes 结构

```rust
#[derive(Asset, TypePath, Debug)]
pub struct SkinnedMeshInverseBindposes(Box<[Mat4]>);

impl From<Vec<Mat4>> for SkinnedMeshInverseBindposes {
    fn from(value: Vec<Mat4>) -> Self {
        Self(value.into_boxed_slice())
    }
}

impl Deref for SkinnedMeshInverseBindposes {
    type Target = [Mat4];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
```

**蒙皮动画原理**：

```text
1. 绑定姿势（Bind Pose）：
   - 模型在绑定骨骼时的姿势
   - 每个关节有一个初始变换矩阵

2. 逆绑定姿势（Inverse Bind Pose）：
   - 绑定姿势矩阵的逆矩阵
   - 存储在 SkinnedMeshInverseBindposes 中

3. 蒙皮计算（GPU 着色器）：
   对于每个顶点：
   final_position = sum(
       joint_weight[i] * 
       joint_matrix[i] * 
       inverse_bindpose[i] * 
       original_position
   )
```

**使用示例**：

```rust
use bevy::prelude::*;
use bevy_mesh::skinning::SkinnedMesh;

fn setup_skinned_mesh(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut inverse_bindposes: ResMut<Assets<SkinnedMeshInverseBindposes>>,
) {
    // 创建逆绑定姿势
    let ibp = inverse_bindposes.add(SkinnedMeshInverseBindposes(
        vec![Mat4::IDENTITY; 16].into_boxed_slice(),
    ));
    
    // 创建关节实体
    let joints: Vec<Entity> = (0..16)
        .map(|_| commands.spawn(Transform::default()).id())
        .collect();
    
    // 创建蒙皮网格
    commands.spawn((
        SkinnedMesh {
            inverse_bindposes: ibp,
            joints,
        },
        PbrBundle {
            mesh: meshes.add(Cuboid::default()),
            ..default()
        },
    ));
}
```

---

### 4. 顶点属性管理

**文件**: [`vertex.rs`](file:///d:/work/ttc/bevy/crates/bevy_mesh/src/vertex.rs)

#### MeshVertexAttribute 结构

```rust
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Reflect)]
pub struct MeshVertexAttribute {
    /// 属性名称
    pub name: Cow<'static, str>,
    
    /// 属性 ID
    pub id: u32,
    
    /// 属性格式
    pub format: VertexFormat,
}

impl MeshVertexAttribute {
    pub fn new(name: impl Into<Cow<'static, str>>, id: u32, format: VertexFormat) -> Self {
        MeshVertexAttribute {
            name: name.into(),
            id,
            format,
        }
    }
}
```

#### VertexAttributeValues 枚举

```rust
#[derive(Clone, Debug, PartialEq, Reflect)]
pub enum VertexAttributeValues {
    Float32x1(Vec<f32>),
    Float32x2(Vec<[f32; 2]>),
    Float32x3(Vec<[f32; 3]>),
    Float32x4(Vec<[f32; 4]>),
    Sint32x1(Vec<i32>),
    Sint32x2(Vec<[i32; 2]>),
    Sint32x3(Vec<[i32; 3]>),
    Sint32x4(Vec<[i32; 4]>),
    Uint32x1(Vec<u32>),
    Uint32x2(Vec<[u32; 2]>),
    Uint32x3(Vec<[u32; 3]>),
    Uint32x4(Vec<[u32; 4]>),
    Uint8x2Norm(Vec<[u8; 2]>),
    Uint8x4Norm(Vec<[u8; 4]>),
}
```

---

### 5. 索引管理

**文件**: [`index.rs`](file:///d:/work/ttc/bevy/crates/bevy_mesh/src/index.rs)

#### Indices 枚举

```rust
#[derive(Clone, Debug, PartialEq, Reflect)]
pub enum Indices {
    /// 16 位索引（最多 65536 个顶点）
    U16(Vec<u16>),
    
    /// 32 位索引（最多 4294967296 个顶点）
    U32(Vec<u32>),
}

impl Indices {
    /// 获取索引数量
    pub fn len(&self) -> usize {
        match self {
            Indices::U16(indices) => indices.len(),
            Indices::U32(indices) => indices.len(),
        }
    }
    
    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    
    /// 获取索引格式
    pub fn format(&self) -> IndexFormat {
        match self {
            Indices::U16(_) => IndexFormat::Uint16,
            Indices::U32(_) => IndexFormat::Uint32,
        }
    }
}
```

**索引优化**：
- **U16**：适合小型网格（< 65536 顶点），节省内存
- **U32**：适合大型网格（> 65536 顶点）
- **无索引**：每个三角形独立顶点，浪费内存但简单

---

### 6. 高级网格处理

#### 法线计算

```rust
impl Mesh {
    /// 计算平滑法线（面积加权）
    pub fn compute_smooth_normals(&mut self) -> Result<(), MeshAttributeError> {
        self.compute_area_weighted_normals()
    }
    
    /// 计算面积加权法线
    pub fn compute_area_weighted_normals(&mut self) -> Result<(), MeshAttributeError> {
        self.compute_custom_smooth_normals(|[a, b, c], positions, normals| {
            let normal = triangle_area_normal(positions[a], positions[b], positions[c]);
            for idx in [a, b, c] {
                normals[idx] += normal;
            }
        })
    }
    
    /// 自定义法线计算
    pub fn compute_custom_smooth_normals(
        &mut self,
        per_triangle: impl Fn([usize; 3], &[[f32; 3]], &mut [Vec3]),
    ) -> Result<(), MeshAttributeError> {
        // 1. 获取位置属性
        let positions = self.attribute::<[f32; 3]>(Mesh::ATTRIBUTE_POSITION)?;
        
        // 2. 初始化法线数组
        let mut normals = vec![Vec3::ZERO; positions.len()];
        
        // 3. 遍历每个三角形
        let indices = self.indices()?;
        for triangle_indices in indices.iter().triangles() {
            per_triangle(triangle_indices, positions, &mut normals);
        }
        
        // 4. 归一化法线
        for normal in &mut normals {
            *normal = normal.normalize();
        }
        
        // 5. 设置法线属性
        self.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    }
}
```

**法线计算原理**：

```text
三角形法线 = (v2 - v1) × (v3 - v1)
面积加权 = 三角形法线 × 三角形面积

平滑法线 = 共享顶点的所有三角形法线之和（归一化）

示例：
顶点 A 属于三角形 T1, T2, T3
法线 A = normalize(normal(T1) + normal(T2) + normal(T3))
```

#### 切线生成

```rust
impl Mesh {
    /// 使用 MikkTSpace 生成切线
    pub fn generate_tangents(&mut self) -> Result<(), MeshAttributeError> {
        // 1. 获取位置、法线、UV 属性
        let positions = self.attribute::<[f32; 3]>(Mesh::ATTRIBUTE_POSITION)?;
        let normals = self.attribute::<[f32; 3]>(Mesh::ATTRIBUTE_NORMAL)?;
        let uvs = self.attribute::<[f32; 2]>(Mesh::ATTRIBUTE_UV_0)?;
        
        // 2. 使用 MikkTSpace 库生成切线
        let tangents = mikktspace::generate_tangents(
            positions,
            normals,
            uvs,
            self.indices()?,
        )?;
        
        // 3. 设置切线属性
        self.insert_attribute(Mesh::ATTRIBUTE_TANGENT, tangents)
    }
}
```

**切线用途**：
- **法线贴图**：切线空间用于正确采样法线贴图
- **视差贴图**：切线空间用于高级视差效果
- **自阴影**：切线空间用于模拟表面自阴影

---

### 7. 序列化

**文件**: [`mesh.rs`](file:///d:/work/ttc/bevy/crates/bevy_mesh/src/mesh.rs)（serialize feature）

#### SerializedMesh 结构

```rust
#[cfg(feature = "serialize")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedMesh {
    primitive_topology: PrimitiveTopology,
    attributes: Vec<(MeshVertexAttributeId, SerializedMeshAttributeData)>,
    indices: Option<Indices>,
}

#[cfg(feature = "serialize")]
impl SerializedMesh {
    /// 从 Mesh 创建 SerializedMesh
    pub fn from_mesh(mesh: &Mesh) -> Result<Self, MeshAccessError> {
        let attributes = mesh.attributes()?
            .iter()
            .map(|(id, data)| (*id, data.into()))
            .collect();
        
        let indices = mesh.indices()?.cloned();
        
        Ok(Self {
            primitive_topology: mesh.primitive_topology(),
            attributes,
            indices,
        })
    }
    
    /// 反序列化为 Mesh
    pub fn deserialize(&self) -> Result<Mesh, MeshAttributeError> {
        let mut mesh = Mesh::new(self.primitive_topology, RenderAssetUsages::default());
        
        for (id, data) in &self.attributes {
            mesh.insert_attribute(*id, data.clone())?;
        }
        
        if let Some(indices) = &self.indices {
            mesh.insert_indices(indices.clone())?;
        }
        
        Ok(mesh)
    }
}
```

**注意事项**：
- **短期传输**：仅用于进程间传输，不适合长期存储
- **版本依赖**：不同 Bevy 版本不兼容
- **信息丢失**：仅保留基本信息（拓扑、属性、索引）

---

## 典型使用示例

### 1. 手动创建网格

```rust
use bevy::prelude::*;
use bevy_mesh::{Mesh, Indices, PrimitiveTopology};

fn create_custom_mesh() -> Mesh {
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        // 添加 4 个顶点位置
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![
                [0.0, 0.0, 0.0],  // 顶点 0
                [1.0, 0.0, 0.0],  // 顶点 1
                [1.0, 1.0, 0.0],  // 顶点 2
                [0.0, 1.0, 0.0],  // 顶点 3
            ],
        )
        // 添加 UV 坐标
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_UV_0,
            vec![
                [0.0, 1.0],  // 左上角
                [1.0, 1.0],  // 右上角
                [1.0, 0.0],  // 右下角
                [0.0, 0.0],  // 左下角
            ],
        )
        // 添加法线
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_NORMAL,
            vec![
                [0.0, 0.0, 1.0],  // 全部指向 Z 轴正方向
                [0.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
            ],
        )
        // 添加索引（2 个三角形）
        .with_inserted_indices(Indices::U32(vec![
            0, 1, 2,  // 第一个三角形
            0, 2, 3,  // 第二个三角形
        ]))
}
```

### 2. 修改现有网格

```rust
use bevy::prelude::*;
use bevy_mesh::Mesh;

fn modify_mesh(mesh: &mut Mesh) {
    // 获取位置属性
    let mut positions = mesh
        .attribute_mut::<[f32; 3]>(Mesh::ATTRIBUTE_POSITION)
        .expect("Mesh has no position attribute");
    
    // 修改 Y 坐标（向上移动）
    for position in &mut positions {
        position[1] += 1.0;
    }
    
    // 计算法线
    mesh.compute_smooth_normals().unwrap();
    
    // 生成切线
    mesh.generate_tangents().unwrap();
}
```

### 3. 组合多个图元

```rust
use bevy::prelude::*;
use bevy_math::primitives::{Cuboid, Sphere};
use bevy_mesh::Meshable;

fn create_complex_mesh(mut meshes: ResMut<Assets<Mesh>>) {
    // 创建多个图元
    let cube = Cuboid::default().mesh().build();
    let sphere = Sphere::default().mesh().build();
    
    // 组合（需要手动合并顶点和索引）
    let mut combined = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    
    // 合并位置
    let mut positions = Vec::new();
    positions.extend(cube.attribute::<[f32; 3]>(Mesh::ATTRIBUTE_POSITION).unwrap());
    positions.extend(sphere.attribute::<[f32; 3]>(Mesh::ATTRIBUTE_POSITION).unwrap());
    combined.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions).unwrap();
    
    // 合并索引（需要偏移）
    let cube_indices = cube.indices().unwrap();
    let sphere_indices = sphere.indices().unwrap();
    let cube_vertex_count = cube.attribute::<[f32; 3]>(Mesh::ATTRIBUTE_POSITION).unwrap().len();
    
    let mut combined_indices = Vec::new();
    combined_indices.extend(cube_indices.iter().map(|i| i as u32));
    combined_indices.extend(sphere_indices.iter().map(|i| i as u32 + cube_vertex_count as u32));
    combined.insert_indices(Indices::U32(combined_indices)).unwrap();
    
    meshes.add(combined);
}
```

---

## 设计特点

### 1. 灵活性
- **自定义属性**：支持任意顶点属性
- **多种格式**：支持 14 种顶点格式
- **动态构建**：运行时修改网格数据

### 2. 性能优化
- **BTreeMap**：稳定的迭代顺序，避免 GPU 重新编译
- **Extractable Data**：避免重复数据处理
- **索引优化**：支持 U16/U32 索引

### 3. 可扩展性
- **Meshable 特质**：易于添加新图元
- **自定义属性**：支持游戏特定数据
- **插件系统**：MeshPlugin 集成到 App

### 4. 正确性
- **错误处理**：明确的错误类型和信息
- **验证**：构建时验证数据一致性
- **文档**：详细的 API 文档和示例

---

## 常见问题

### 1. UV 坐标方向

```text
Bevy UV 坐标：
(0.0, 0.0) → 左上角
(1.0, 1.0) → 右下角

OpenGL UV 坐标：
(0.0, 0.0) → 左下角
(1.0, 1.0) → 右上角

注意：Bevy 与 OpenGL 不同！
```

### 2. 顶点缠绕顺序

```text
默认 CullMode = Back（剔除背面）
正面 = 顶点逆时针顺序

示例：
三角形 (0, 1, 2) 是正面
三角形 (0, 2, 1) 是背面（会被剔除）
```

### 3. AABB 更新

```text
Bevy 自动计算新网格的 AABB
修改网格后需要手动更新：

fn update_mesh_aabb(mut query: Query<(&Mesh, &mut Aabb)>) {
    for (mesh, mut aabb) in &mut query {
        *aabb = mesh.compute_aabb().unwrap();
    }
}
```

### 4. 网格提取

```text
Mesh 数据会被提取到 RenderWorld
提取后无法在 MainWorld 访问：

错误：MeshAccessError::ExtractedToRenderWorld

解决：设置 asset_usage = RenderAssetUsages::MAIN_WORLD
```

---

## 性能优化建议

### 1. 减少顶点数量
```rust
// 使用低分辨率图元
let low_res_sphere = Sphere::default()
    .mesh()
    .resolution(16)  // 默认 32
    .build();
```

### 2. 使用索引
```rust
// 有索引 vs 无索引
// 有索引：8 顶点 + 36 索引 = 8×12 + 36×4 = 240 字节
// 无索引：36 顶点 = 36×12 = 432 字节
// 节省：~44%
```

### 3. 选择合适的索引格式
```rust
// U16（< 65536 顶点）：节省 50% 内存
// U32（> 65536 顶点）：支持大型网格

let indices = if vertex_count < 65536 {
    Indices::U16(indices)
} else {
    Indices::U32(indices)
};
```

### 4. 预计算数据
```rust
// 预计算法线、切线
mesh.compute_smooth_normals().unwrap();
mesh.generate_tangents().unwrap();

// 避免运行时计算
```

---

## 文件结构

```
src/
├── mesh.rs                    # 核心 Mesh 结构
├── vertex.rs                  # 顶点属性管理
├── index.rs                   # 索引管理
├── components.rs              # ECS 组件（Mesh2d, Mesh3d）
├── skinning.rs                # 蒙皮动画
├── morph.rs                   # 变形动画
├── mikktspace.rs              # 切线生成
├── conversions.rs             # 类型转换
├── primitives/                # 图元生成
│   ├── mod.rs
│   ├── dim2.rs                # 2D 图元
│   ├── dim3/                  # 3D 图元
│   │   ├── capsule.rs
│   │   ├── cone.rs
│   │   ├── conical_frustum.rs
│   │   ├── cuboid.rs
│   │   ├── cylinder.rs
│   │   ├── mod.rs
│   │   ├── plane.rs
│   │   ├── polyline3d.rs
│   │   ├── segment3d.rs
│   │   ├── sphere.rs
│   │   ├── tetrahedron.rs
│   │   ├── torus.rs
│   │   └── triangle3d.rs
│   └── extrusion.rs           # 挤出
└── lib.rs                     # 主入口和 MeshPlugin
```

---

## 总结

`bevy_mesh` 是一个**功能完整、灵活高效的网格数据模块**，具有以下优势：

**核心优势**：
1. **灵活的数据模型**：支持自定义顶点属性和格式
2. **丰富的图元库**：12+ 种 3D 图元和 2D 图元
3. **高级处理功能**：法线计算、切线生成、蒙皮动画
4. **性能优化**：BTreeMap、Extractable Data、索引优化
5. **易于扩展**：Meshable 特质、自定义属性

**适用场景**：
- 3D 游戏开发
- 建筑可视化
- 科学可视化
- CAD 应用

**学习资源**：
- [Bevy Mesh 文档](https://docs.rs/bevy/latest/bevy/mesh/index.html)
- [Bevy 示例](https://github.com/bevyengine/bevy/tree/main/examples)
- [Real-time Rendering Book](https://www.realtimerendering.com/)

---

**注意**：`bevy_mesh` 是底层数据结构，与 `bevy_render` 和 `bevy_pbr` 紧密集成。理解网格数据对于优化渲染性能至关重要。
