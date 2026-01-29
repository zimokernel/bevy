# bevy_asset 模块总结

## 概述

`bevy_asset` 是 Bevy 游戏引擎的**核心资源管理系统**，提供了完整的异步资源加载、缓存、热重载和资源处理功能。它是实现高性能、可扩展资源管理的关键组件。

**核心特性**：
- **异步资源加载**：非阻塞式资源加载，支持后台任务
- **类型安全**：强类型资源系统，编译时类型检查
- **引用计数**：自动资源生命周期管理
- **热重载**：开发时自动检测并重新加载修改的资源
- **资源处理**：自动资源转换、压缩、优化
- **多源支持**：文件系统、嵌入资源、网络资源
- **依赖管理**：自动处理资源之间的依赖关系

**设计目标**：
- **高性能**：最小化内存占用和加载时间
- **可扩展**：支持自定义资源类型和加载器
- **易用性**：简单的 API，自动处理复杂细节
- **鲁棒性**：崩溃恢复、事务日志

---

## 核心架构

```
Asset System（资源系统）
├── AssetServer（资源服务器）
│   ├── Asset Loading（资源加载）
│   ├── Asset Caching（资源缓存）
│   ├── Dependency Management（依赖管理）
│   └── Hot Reloading（热重载）
├── Assets<T>（资源集合）
│   ├── Storage（存储）
│   ├── Reference Counting（引用计数）
│   └── Event System（事件系统）
├── Handle<T>（资源句柄）
│   ├── Strong Handle（强句柄）
│   ├── Weak Handle（弱句柄）
│   └── Untyped Handle（无类型句柄）
├── AssetLoader（资源加载器）
│   ├── Async Loading（异步加载）
│   ├── Settings（设置）
│   └── Error Handling（错误处理）
├── AssetProcessor（资源处理器）
│   ├── Asset Transforming（资源转换）
│   ├── Caching（缓存）
│   └── Transaction Log（事务日志）
└── Asset I/O（资源输入输出）
    ├── AssetReader（资源读取器）
    ├── AssetWriter（资源写入器）
    ├── AssetSource（资源源）
    └── AssetWatcher（资源监视器）
```

**关键设计**：
- **分层架构**：资源服务器、资源集合、句柄、加载器、处理器
- **异步优先**：所有 I/O 操作都是异步的
- **类型安全**：泛型资源类型，编译时检查
- **引用计数**：自动资源清理，避免内存泄漏
- **事件驱动**：资源加载、修改、卸载都产生事件

---

## 主要子模块

### 模块功能总览

| 模块 | 功能 | 关键组件 |
|------|------|----------|
| **server/** | 资源服务器 | `AssetServer`, `LoadContext`, `LoadState` |
| **assets.rs** | 资源集合 | `Assets<T>`, `Entry<T>`, `AssetEvent` |
| **handle.rs** | 资源句柄 | `Handle<T>`, `UntypedHandle`, `StrongHandle` |
| **loader.rs** | 资源加载器 | `AssetLoader`, `LoadContext`, `LoadedAsset` |
| **processor/** | 资源处理器 | `AssetProcessor`, `Process`, `AssetTransformer` |
| **io/** | 资源 I/O | `AssetReader`, `AssetWriter`, `AssetSource` |
| **saver.rs** | 资源保存器 | `AssetSaver`, `SaveContext` |
| **transformer.rs** | 资源转换器 | `AssetTransformer`, `TransformContext` |
| **meta.rs** | 资源元数据 | `AssetMeta`, `AssetHash`, `MetaTransform` |
| **event.rs** | 资源事件 | `AssetEvent`, `AssetAdded`, `AssetModified` |

---

## 核心子模块详解

### 1. Asset 系统

**文件**: [`lib.rs`](file:///d:/work/ttc/bevy/crates/bevy_asset/src/lib.rs)

#### Asset 特质定义

```rust
/// A trait for types that can be loaded by the asset server.
///
/// # Derive
///
/// This trait can be derived with the `#[derive(Asset)]` macro.
///
/// # Example
///
/// ```rust
/// use bevy_asset::Asset;
/// use bevy_reflect::TypePath;
///
/// #[derive(Asset, TypePath)]
/// struct MyAsset {
///     value: u32,
/// }
/// ```
pub trait Asset: Send + Sync + TypePath + VisitAssetDependencies + 'static {
    /// The type of the asset's ID.
    type Id: AssetId = Uuid;
}

/// This trait defines how to visit the dependencies of an asset.
/// For example, a 3D model might require both textures and meshes to be loaded.
///
/// Note that this trait is automatically implemented when deriving [`Asset`].
pub trait VisitAssetDependencies {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId));
}
```

**Asset 特质要求**：

| 要求 | 说明 |
|------|------|
| `Send + Sync` | 线程安全，支持多线程加载 |
| `TypePath` | 类型路径，用于反射和序列化 |
| `VisitAssetDependencies` | 访问资源依赖 |
| `'static` | 静态生命周期 |

**派生宏**：

```rust
#[derive(Asset, Reflect, TypePath)]
struct MyAsset {
    // 普通字段
    name: String,
    
    // 使用 #[dependency] 标记依赖
    #[dependency]
    texture: Handle<Texture>,
    
    // Vec 中的依赖
    #[dependency]
    meshes: Vec<Handle<Mesh>>,
    
    // Option 中的依赖
    #[dependency]
    optional_material: Option<Handle<Material>>,
}
```

**依赖管理**：

```rust
impl VisitAssetDependencies for MyAsset {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        // 访问 texture 依赖
        visit(self.texture.id().untyped());
        
        // 访问 meshes 依赖
        for mesh in &self.meshes {
            visit(mesh.id().untyped());
        }
        
        // 访问 optional_material 依赖
        if let Some(material) = &self.optional_material {
            visit(material.id().untyped());
        }
    }
}
```

**使用示例**：

```rust
use bevy_asset::Asset;
use bevy_reflect::TypePath;

// 简单资源类型
#[derive(Asset, TypePath, Default)]
struct Texture {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

// 带依赖的资源类型
#[derive(Asset, TypePath)]
struct Material {
    albedo: Color,
    #[dependency]
    albedo_texture: Option<Handle<Texture>>,
    #[dependency]
    normal_texture: Option<Handle<Texture>>,
    #[dependency]
    metallic_roughness_texture: Option<Handle<Texture>>,
}

// 复杂资源类型
#[derive(Asset, TypePath)]
struct Scene {
    name: String,
    #[dependency]
    meshes: Vec<Handle<Mesh>>,
    #[dependency]
    materials: Vec<Handle<Material>>,
    #[dependency]
    textures: Vec<Handle<Texture>>,
    entities: Vec<EntityData>,
}
```

---

### 2. Handle 系统

**文件**: [`handle.rs`](file:///d:/work/ttc/bevy/crates/bevy_asset/src/handle.rs)

#### Handle 结构定义

```rust
/// A reference-counted handle to an asset.
///
/// `Handle<T>` is the primary way to reference assets in Bevy.
/// It is cheap to clone and can be safely shared between threads.
///
/// # Reference Counting
///
/// Handles are reference-counted:
/// - Cloning a handle increments the reference count
/// - Dropping a handle decrements the reference count
/// - When the reference count reaches zero, the asset is removed from memory
///
/// # Strong vs Weak Handles
///
/// - **Strong Handle**: Increases the reference count
/// - **Weak Handle**: Does not increase the reference count
///
/// Use strong handles for assets that must stay in memory.
/// Use weak handles for assets that can be unloaded when not in use.
#[derive(Component, Clone, PartialEq, Eq, Hash, Reflect)]
#[reflect(Component, PartialEq, Hash)]
pub struct Handle<T: Asset> {
    // 内部实现细节
    // ...
}

/// An untyped handle to an asset.
///
/// Similar to `Handle<T>`, but without type information.
/// Useful when you need to store handles of different types.
#[derive(Component, Clone, PartialEq, Eq, Hash, Reflect)]
#[reflect(Component, PartialEq, Hash)]
pub struct UntypedHandle {
    // 内部实现细节
    // ...
}
```

**Handle 类型**：

```text
Handle<T>
├── StrongHandle<T>（强句柄）
│   ├── 增加引用计数
│   ├── 保证资源存在
│   └── 用于必须保留的资源
├── WeakHandle<T>（弱句柄）
│   ├── 不增加引用计数
│   ├── 资源可能被卸载
│   └── 用于可选资源
└── UntypedHandle（无类型句柄）
    ├── 不包含类型信息
    ├── 可存储不同类型的资源
    └── 需要运行时类型检查
```

**Handle 方法**：

```rust
impl<T: Asset> Handle<T> {
    /// 获取资源 ID
    pub fn id(&self) -> AssetId<T> {
        // ...
    }
    
    /// 检查是否为强句柄
    pub fn is_strong(&self) -> bool {
        // ...
    }
    
    /// 检查是否为弱句柄
    pub fn is_weak(&self) -> bool {
        // ...
    }
    
    /// 转换为弱句柄
    pub fn downgrade(&self) -> WeakHandle<T> {
        // ...
    }
    
    /// 转换为无类型句柄
    pub fn untyped(&self) -> UntypedHandle {
        // ...
    }
    
    /// 尝试升级弱句柄为强句柄
    pub fn upgrade(&self) -> Option<Handle<T>> {
        // ...
    }
}
```

**引用计数机制**：

```text
创建资源：
    asset = Assets::add(asset_data)
    reference_count = 1

克隆强句柄：
    handle2 = handle1.clone()
    reference_count += 1  // reference_count = 2

丢弃强句柄：
    drop(handle1)
    reference_count -= 1  // reference_count = 1

丢弃所有强句柄：
    drop(handle2)
    reference_count -= 1  // reference_count = 0
    资源从内存中移除

弱句柄：
    weak_handle = handle.downgrade()
    reference_count 不变
    
    upgrade_result = weak_handle.upgrade()
    if 资源存在:
        返回 Some(strong_handle)
        reference_count += 1
    else:
        返回 None
```

**Handle 使用示例**：

```rust
use bevy_asset::{Asset, Handle, UntypedHandle};
use bevy_ecs::prelude::*;

// 强句柄示例
fn setup_strong_handle(mut commands: Commands, asset_server: Res<AssetServer>) {
    // 加载纹理（返回强句柄）
    let texture_handle: Handle<Texture> = asset_server.load("textures/player.png");
    
    // 克隆强句柄（增加引用计数）
    let texture_handle_clone = texture_handle.clone();
    
    // 存储强句柄（资源将保持在内存中）
    commands.spawn((
        SpriteBundle {
            texture: texture_handle,
            ..default()
        },
    ));
    
    // 存储另一个强句柄
    commands.spawn((
        SpriteBundle {
            texture: texture_handle_clone,
            ..default()
        },
    ));
    
    // 两个精灵使用同一个纹理
    // 引用计数 = 2
}

// 弱句柄示例
fn setup_weak_handle(mut commands: Commands, asset_server: Res<AssetServer>) {
    // 加载纹理
    let strong_handle: Handle<Texture> = asset_server.load("textures/background.png");
    
    // 转换为弱句柄
    let weak_handle = strong_handle.downgrade();
    
    // 存储强句柄（资源保持在内存中）
    commands.spawn((
        Background {
            texture: strong_handle,
        },
    ));
    
    // 存储弱句柄（不增加引用计数）
    commands.spawn((
        BackgroundPreview {
            texture: weak_handle,
        },
    ));
    
    // 引用计数 = 1（仅强句柄）
}

// 升级弱句柄示例
fn use_weak_handle(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut query: Query<&mut BackgroundPreview>,
    mut textures: ResMut<Assets<Texture>>,
) {
    for mut preview in &mut query {
        // 尝试升级弱句柄
        if let Some(strong_handle) = preview.texture.upgrade() {
            // 资源存在，可以使用
            let texture = textures.get(&strong_handle).unwrap();
            println!("Texture size: {}x{}", texture.width, texture.height);
        } else {
            // 资源已被卸载
            println!("Texture has been unloaded");
        }
    }
}

// 无类型句柄示例
fn setup_untyped_handles(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    // 加载不同类型的资源
    let texture_handle: Handle<Texture> = asset_server.load("textures/icon.png");
    let mesh_handle: Handle<Mesh> = asset_server.load("meshes/cube.glb");
    let shader_handle: Handle<Shader> = asset_server.load("shaders/custom.wgsl");
    
    // 转换为无类型句柄
    let untyped_texture: UntypedHandle = texture_handle.clone().untyped();
    let untyped_mesh: UntypedHandle = mesh_handle.clone().untyped();
    let untyped_shader: UntypedHandle = shader_handle.clone().untyped();
    
    // 存储在 Vec 中
    let handles: Vec<UntypedHandle> = vec![
        untyped_texture,
        untyped_mesh,
        untyped_shader,
    ];
    
    // 存储到资源中
    commands.insert_resource(ResourceHandles { handles });
}
```

**常见问题**：

```text
问题 1: 资源突然消失

原因：
- 所有强句柄都被丢弃
- 引用计数变为 0
- 资源被从内存中移除

解决方法：
- 确保至少有一个强句柄保留
- 使用资源存储强句柄
- 避免过度使用弱句柄

问题 2: 内存泄漏

原因：
- 强句柄永远不被丢弃
- 资源永远保留在内存中

解决方法：
- 及时丢弃不再需要的强句柄
- 使用弱句柄缓存
- 按关卡/区域管理资源生命周期

问题 3: 弱句柄升级失败

原因：
- 资源已被卸载
- 所有强句柄都被丢弃

解决方法：
- 在升级前检查资源是否存在
- 重新加载资源
- 确保至少有一个强句柄保留
```

---

### 3. Assets<T> 资源集合

**文件**: [`assets.rs`](file:///d:/work/ttc/bevy/crates/bevy_asset/src/assets.rs)

#### Assets<T> 结构定义

```rust
/// A collection of assets of type `T`.
///
/// `Assets<T>` is a resource that stores all loaded assets of type `T`.
/// It is automatically created when you register an asset type.
///
/// # Usage
///
/// ```rust
/// use bevy_asset::{Asset, Assets};
/// use bevy_ecs::prelude::*;
///
/// fn access_asset(
///     textures: Res<Assets<Texture>>,
///     texture_handle: Res<Handle<Texture>>,
/// ) {
///     // 获取资源
///     if let Some(texture) = textures.get(texture_handle) {
///         println!("Texture size: {}x{}", texture.width, texture.height);
///     }
/// }
/// ```
#[derive(Resource)]
pub struct Assets<T: Asset> {
    // 内部存储
    // ...
}

impl<T: Asset> Assets<T> {
    /// 添加新资源
    pub fn add(&mut self, asset: T) -> Handle<T> {
        // ...
    }
    
    /// 获取资源引用
    pub fn get(&self, handle: &Handle<T>) -> Option<&T> {
        // ...
    }
    
    /// 获取可变资源引用
    pub fn get_mut(&mut self, handle: &Handle<T>) -> Option<&mut T> {
        // ...
    }
    
    /// 移除资源
    pub fn remove(&mut self, handle: &Handle<T>) -> Option<T> {
        // ...
    }
    
    /// 检查资源是否存在
    pub fn contains(&self, handle: &Handle<T>) -> bool {
        // ...
    }
    
    /// 获取所有资源的迭代器
    pub fn iter(&self) -> impl Iterator<Item = (Handle<T>, &T)> {
        // ...
    }
    
    /// 获取所有资源的可变迭代器
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Handle<T>, &mut T)> {
        // ...
    }
    
    /// 获取资源数量
    pub fn len(&self) -> usize {
        // ...
    }
    
    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        // ...
    }
}
```

**Assets<T> 事件**：

```rust
/// An event related to an asset.
///
/// Events are emitted when:
/// - An asset is added
/// - An asset is modified
/// - An asset is removed
#[derive(Event, Debug, Clone)]
pub enum AssetEvent<T: Asset> {
    /// An asset was added
    Added { handle: Handle<T> },
    
    /// An asset was modified
    Modified { handle: Handle<T> },
    
    /// An asset was removed
    Removed { handle: Handle<T> },
}

// 监听资源事件
fn listen_to_asset_events(
    mut texture_events: EventReader<AssetEvent<Texture>>,
) {
    for event in texture_events.iter() {
        match event {
            AssetEvent::Added { handle } => {
                println!("Texture added: {:?}", handle);
            }
            AssetEvent::Modified { handle } => {
                println!("Texture modified: {:?}", handle);
            }
            AssetEvent::Removed { handle } => {
                println!("Texture removed: {:?}", handle);
            }
        }
    }
}
```

**Assets<T> 使用示例**：

```rust
use bevy_asset::{Asset, Assets, Handle};
use bevy_ecs::prelude::*;

// 访问资源
fn access_asset(
    textures: Res<Assets<Texture>>,
    query: Query<&Handle<Texture>>,
) {
    for handle in &query {
        if let Some(texture) = textures.get(handle) {
            println!("Texture: {}x{}", texture.width, texture.height);
        } else {
            println!("Texture not loaded yet");
        }
    }
}

// 修改资源
fn modify_asset(
    mut textures: ResMut<Assets<Texture>>,
    query: Query<&Handle<Texture>>,
) {
    for handle in &query {
        if let Some(mut texture) = textures.get_mut(handle) {
            // 修改资源数据
            texture.width = 1024;
            texture.height = 1024;
            
            // 这会触发 AssetEvent::Modified
        }
    }
}

// 程序化创建资源
fn create_procedural_asset(
    mut commands: Commands,
    mut textures: ResMut<Assets<Texture>>,
) {
    // 创建资源数据
    let texture_data = Texture {
        width: 512,
        height: 512,
        data: vec![255; 512 * 512 * 4],  // RGBA 数据
        format: TextureFormat::Rgba8UnormSrgb,
    };
    
    // 添加到资源集合
    let handle: Handle<Texture> = textures.add(texture_data);
    
    // 使用资源
    commands.spawn((
        SpriteBundle {
            texture: handle,
            ..default()
        },
    ));
}

// 批量操作资源
fn batch_operation(
    mut textures: ResMut<Assets<Texture>>,
) {
    // 迭代所有资源
    for (handle, texture) in textures.iter() {
        println!("Texture {:?}: {}x{}", handle, texture.width, texture.height);
    }
    
    // 批量修改
    for (handle, mut texture) in textures.iter_mut() {
        if texture.width > 2048 {
            texture.width = 2048;
            texture.height = 2048;
        }
    }
    
    // 批量移除
    let to_remove: Vec<_> = textures
        .iter()
        .filter(|(_, texture)| texture.data.is_empty())
        .map(|(handle, _)| handle)
        .collect();
    
    for handle in to_remove {
        textures.remove(&handle);
    }
}
```

---

### 4. AssetServer 资源服务器

**文件**: [`server/mod.rs`](file:///d:/work/ttc/bevy/crates/bevy_asset/src/server/mod.rs)

#### AssetServer 结构定义

```rust
/// The main entry point for loading assets.
///
/// `AssetServer` coordinates the loading of assets from various sources.
/// It handles:
/// - Asset loading from disk
/// - Asset caching
/// - Dependency management
/// - Hot reloading
/// - Load state tracking
///
/// # Usage
///
/// ```rust
/// use bevy_asset::{AssetServer, Handle};
/// use bevy_ecs::prelude::*;
///
/// fn setup(asset_server: Res<AssetServer>) {
///     // 加载资源
///     let texture_handle: Handle<Texture> = asset_server.load("textures/player.png");
///     let mesh_handle: Handle<Mesh> = asset_server.load("meshes/cube.glb");
///     
///     // 检查加载状态
///     if asset_server.is_loaded(texture_handle) {
///         println!("Texture loaded");
///     }
/// }
/// ```
#[derive(Resource)]
pub struct AssetServer {
    // 内部实现
    // ...
}

impl AssetServer {
    /// 加载资源
    pub fn load<'a, A: Asset>(&self, path: impl Into<AssetPath<'a>>) -> Handle<A> {
        // ...
    }
    
    /// 异步加载资源
    pub async fn load_async<'a, A: Asset>(&self, path: impl Into<AssetPath<'a>>) -> Handle<A> {
        // ...
    }
    
    /// 检查资源是否加载
    pub fn is_loaded<A: Asset>(&self, handle: Handle<A>) -> bool {
        // ...
    }
    
    /// 检查资源及其依赖是否加载
    pub fn is_loaded_with_dependencies<A: Asset>(&self, handle: Handle<A>) -> bool {
        // ...
    }
    
    /// 获取加载状态
    pub fn load_state<A: Asset>(&self, handle: Handle<A>) -> LoadState {
        // ...
    }
    
    /// 重新加载资源
    pub fn reload<A: Asset>(&self, handle: Handle<A>) {
        // ...
    }
    
    /// 从 UUID 获取句柄
    pub fn get_handle<A: Asset>(&self, uuid: Uuid) -> Option<Handle<A>> {
        // ...
    }
}
```

**LoadState 枚举**：

```rust
/// The loading state of an asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LoadState {
    /// The asset has not been loaded yet.
    NotLoaded,
    
    /// The asset is currently loading.
    Loading,
    
    /// The asset is loaded.
    Loaded,
    
    /// The asset failed to load.
    Failed,
}
```

**AssetServer 使用示例**：

```rust
use bevy_asset::{AssetServer, Handle, LoadState};
use bevy_ecs::prelude::*;
use std::collections::HashSet;

// 简单加载示例
fn simple_load(asset_server: Res<AssetServer>) {
    // 加载纹理
    let texture_handle: Handle<Texture> = asset_server.load("textures/player.png");
    
    // 加载模型（自动处理依赖）
    let scene_handle: Handle<Scene> = asset_server.load("scenes/level1.glb");
    
    // 加载着色器
    let shader_handle: Handle<Shader> = asset_server.load("shaders/pbr.wgsl");
}

// 等待资源加载示例
#[derive(Resource)]
struct LoadingAssets {
    textures: HashSet<Handle<Texture>>,
    meshes: HashSet<Handle<Mesh>>,
}

fn start_loading(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    // 加载多个资源
    let mut textures = HashSet::new();
    textures.insert(asset_server.load("textures/player.png"));
    textures.insert(asset_server.load("textures/enemy.png"));
    textures.insert(asset_server.load("textures/background.png"));
    
    let mut meshes = HashSet::new();
    meshes.insert(asset_server.load("meshes/player.glb"));
    meshes.insert(asset_server.load("meshes/enemy.glb"));
    
    // 存储到资源
    commands.insert_resource(LoadingAssets { textures, meshes });
    
    // 切换到加载状态
    commands.insert_resource(NextState(GameState::Loading));
}

fn check_loading(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut loading_assets: ResMut<LoadingAssets>,
) {
    // 检查所有纹理是否加载
    let all_textures_loaded = loading_assets
        .textures
        .iter()
        .all(|handle| asset_server.is_loaded_with_dependencies(handle.clone()));
    
    // 检查所有模型是否加载
    let all_meshes_loaded = loading_assets
        .meshes
        .iter()
        .all(|handle| asset_server.is_loaded_with_dependencies(handle.clone()));
    
    // 如果所有资源都加载完成
    if all_textures_loaded && all_meshes_loaded {
        // 切换到游戏状态
        commands.insert_resource(NextState(GameState::Playing));
        
        // 清理加载资源
        commands.remove_resource::<LoadingAssets>();
    }
}

// 异步加载示例
async fn async_loading(asset_server: Res<AssetServer>) {
    // 异步加载资源
    let texture_handle: Handle<Texture> = asset_server
        .load_async("textures/large_texture.png")
        .await;
    
    // 资源已加载
    assert!(asset_server.is_loaded(texture_handle));
}

// 热重载示例
fn setup_hot_reload(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    // 加载可热重载的资源
    let shader_handle: Handle<Shader> = asset_server.load("shaders/custom.wgsl");
    
    // 当着色器文件被修改时，会自动重新加载
    // 所有使用该着色器的材质都会自动更新
    
    commands.spawn((
        PbrBundle {
            material: asset_server.load("materials/custom.material"),
            ..default()
        },
    ));
}

// 重新加载示例
fn reload_asset(
    asset_server: Res<AssetServer>,
    query: Query<&Handle<Texture>>,
) {
    for handle in &query {
        // 重新加载资源
        asset_server.reload(handle.clone());
        
        // 资源会被重新加载并触发 AssetEvent::Modified
    }
}
```

---

### 5. AssetLoader 资源加载器

**文件**: [`loader.rs`](file:///d:/work/ttc/bevy/crates/bevy_asset/src/loader.rs)

#### AssetLoader 特质定义

```rust
/// A trait for loading assets from bytes.
///
/// Implement `AssetLoader` to define how to load a custom asset type.
///
/// # Example
///
/// ```rust
/// use bevy_asset::{AssetLoader, LoadContext, LoadedAsset};
/// use bevy_ecs::prelude::*;
/// use std::io::Cursor;
///
/// #[derive(Default)]
/// struct MyAssetLoader;
///
/// impl AssetLoader for MyAssetLoader {
///     type Asset = MyAsset;
///     type Settings = ();
///     type Error = std::io::Error;
///     
///     async fn load(
///         &self,
///         bytes: &[u8],
///         load_context: &mut LoadContext,
///         _settings: &Self::Settings,
///     ) -> Result<Self::Asset, Self::Error> {
///         // 解析字节数据
///         let data = Cursor::new(bytes);
///         let asset = MyAsset::from_reader(data)?;
///         
///         Ok(asset)
///     }
/// }
/// ```
pub trait AssetLoader: Send + Sync + 'static {
    /// The asset type that this loader loads.
    type Asset: Asset;
    
    /// Settings for this loader.
    type Settings: Default + Serialize + for<'a> Deserialize<'a>;
    
    /// Error type for this loader.
    type Error: std::error::Error + Send + Sync + 'static;
    
    /// Load an asset from bytes.
    async fn load(
        &self,
        bytes: &[u8],
        load_context: &mut LoadContext,
        settings: &Self::Settings,
    ) -> Result<Self::Asset, Self::Error>;
    
    /// Get the file extensions that this loader supports.
    fn extensions(&self) -> &[&str];
}
```

**LoadContext 结构**：

```rust
/// A context for loading assets.
///
/// `LoadContext` provides access to:
/// - Loading dependencies
/// - Setting asset metadata
/// - Finishing the asset load
pub struct LoadContext<'a> {
    // 内部实现
    // ...
}

impl<'a> LoadContext<'a> {
    /// 加载依赖资源
    pub fn load<B: Asset>(&mut self, path: impl Into<AssetPath<'_>>) -> Handle<B> {
        // ...
    }
    
    /// 完成资源加载
    pub fn finish<A: Asset>(self, asset: A) -> LoadedAsset<A> {
        // ...
    }
    
    /// 开始标记资源加载
    pub fn begin_labeled_asset(&self) -> LoadContext<'_> {
        // ...
    }
    
    /// 添加已加载的标记资源
    pub fn add_loaded_labeled_asset<A: Asset>(&mut self, label: String, asset: LoadedAsset<A>) {
        // ...
    }
}
```

**AssetLoader 实现示例**：

```rust
use bevy_asset::{Asset, AssetLoader, LoadContext, LoadedAsset};
use bevy_reflect::TypePath;
use serde::{Deserialize, Serialize};
use std::io::Cursor;

// 定义资源类型
#[derive(Asset, TypePath, Debug, Clone)]
struct CustomLevel {
    name: String,
    width: u32,
    height: u32,
    tiles: Vec<u8>,
    #[dependency]
    tileset: Handle<Texture>,
}

// 定义加载器设置
#[derive(Default, Serialize, Deserialize)]
struct CustomLevelLoaderSettings {
    flip_vertical: bool,
    scale: f32,
}

// 定义加载器错误
#[derive(Debug, thiserror::Error)]
enum CustomLevelLoaderError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Parse error: {0}")]
    Parse(String),
    
    #[error("Invalid header")]
    InvalidHeader,
}

// 实现资源加载器
#[derive(Default)]
struct CustomLevelLoader;

impl AssetLoader for CustomLevelLoader {
    type Asset = CustomLevel;
    type Settings = CustomLevelLoaderSettings;
    type Error = CustomLevelLoaderError;
    
    async fn load(
        &self,
        bytes: &[u8],
        load_context: &mut LoadContext,
        settings: &Self::Settings,
    ) -> Result<Self::Asset, Self::Error> {
        // 1. 解析文件头
        let mut reader = Cursor::new(bytes);
        let mut header = [0u8; 4];
        reader.read_exact(&mut header)?;
        
        if &header != b"LVL " {
            return Err(CustomLevelLoaderError::InvalidHeader);
        }
        
        // 2. 读取元数据
        let name_length = reader.read_u32::<LittleEndian>()?;
        let mut name_bytes = vec![0u8; name_length as usize];
        reader.read_exact(&mut name_bytes)?;
        let name = String::from_utf8(name_bytes)
            .map_err(|e| CustomLevelLoaderError::Parse(e.to_string()))?;
        
        let width = reader.read_u32::<LittleEndian>()?;
        let height = reader.read_u32::<LittleEndian>()?;
        
        // 3. 加载依赖资源（tileset）
        let tileset_path = format!("textures/levels/{}/tileset.png", name);
        let tileset_handle: Handle<Texture> = load_context.load(tileset_path);
        
        // 4. 读取瓦片数据
        let tiles_count = width * height;
        let mut tiles = vec![0u8; tiles_count as usize];
        reader.read_exact(&mut tiles)?;
        
        // 5. 应用设置
        if settings.flip_vertical {
            tiles = flip_vertical(&tiles, width, height);
        }
        
        // 6. 创建资源
        let level = CustomLevel {
            name,
            width: (width as f32 * settings.scale) as u32,
            height: (height as f32 * settings.scale) as u32,
            tiles,
            tileset: tileset_handle,
        };
        
        Ok(level)
    }
    
    fn extensions(&self) -> &[&str] {
        &["lvl", "level"]
    }
}

// 辅助函数：垂直翻转瓦片
fn flip_vertical(tiles: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut flipped = vec![0u8; tiles.len()];
    
    for y in 0..height {
        for x in 0..width {
            let src_index = (y * width + x) as usize;
            let dst_index = ((height - 1 - y) * width + x) as usize;
            flipped[dst_index] = tiles[src_index];
        }
    }
    
    flipped
}

// 注册加载器
fn register_loader(app: &mut App) {
    app.register_asset_loader(CustomLevelLoader::default());
}
```

**复杂 AssetLoader 示例**：

```rust
// 加载包含多个子资源的文件
#[derive(Asset, TypePath)]
struct TextureAtlas {
    textures: Vec<Handle<Texture>>,
    regions: Vec<Rect>,
}

#[derive(Default)]
struct TextureAtlasLoader;

impl AssetLoader for TextureAtlasLoader {
    type Asset = TextureAtlas;
    type Settings = ();
    type Error = std::io::Error;
    
    async fn load(
        &self,
        bytes: &[u8],
        load_context: &mut LoadContext,
        _settings: &Self::Settings,
    ) -> Result<Self::Asset, Self::Error> {
        // 解析 atlas 文件（例如 .atlas 格式）
        let atlas_data = parse_atlas_file(bytes)?;
        
        // 加载所有子纹理
        let mut textures = Vec::new();
        let mut regions = Vec::new();
        
        for page in &atlas_data.pages {
            // 加载页面纹理
            let texture_handle: Handle<Texture> = load_context.load(&page.path);
            textures.push(texture_handle);
            
            // 收集该页面的所有区域
            for region in &page.regions {
                regions.push(Rect {
                    min: Vec2::new(region.x, region.y),
                    max: Vec2::new(region.x + region.width, region.y + region.height),
                });
            }
        }
        
        Ok(TextureAtlas { textures, regions })
    }
    
    fn extensions(&self) -> &[&str] {
        &["atlas"]
    }
}
```

---

### 6. AssetProcessor 资源处理器

**文件**: [`processor/mod.rs`](file:///d:/work/ttc/bevy/crates/bevy_asset/src/processor/mod.rs)

#### AssetProcessor 结构定义

```rust
/// A background asset processor.
///
/// `AssetProcessor` automatically transforms assets when they are added or modified.
/// It can be used for:
/// - Texture compression
/// - Audio encoding
/// - Model optimization
/// - Lightmap generation
/// - Any other asset transformation
///
/// # Key Features
///
/// - **Automatic**: Processes assets when they are added or modified
/// - **Configurable**: Supports per-asset settings
/// - **Lossless**: Preserves original assets
/// - **Deterministic**: Same input produces same output
/// - **Crash Resistant**: Uses write-ahead logging
#[derive(Resource, Clone)]
pub struct AssetProcessor {
    server: AssetServer,
    data: Arc<AssetProcessorData>,
}

impl AssetProcessor {
    /// 注册资源处理器
    pub fn register_processor<P: Process>(&mut self, processor: P) {
        // ...
    }
    
    /// 设置默认资源处理器
    pub fn set_default_processor<P: Process>(&mut self, extension: &str, processor: P) {
        // ...
    }
    
    /// 处理资源
    pub async fn process(&self, path: &str) -> Result<(), ProcessError> {
        // ...
    }
    
    /// 处理所有资源
    pub async fn process_all(&self) -> Result<(), ProcessError> {
        // ...
    }
}
```

**Process 特质定义**：

```rust
/// A trait for processing assets.
///
/// Implement `Process` to define how to transform an asset.
///
/// # Example
///
/// ```rust
/// use bevy_asset::{AssetProcessor, Process, LoadedAsset};
/// use bevy_ecs::prelude::*;
///
/// #[derive(Default)]
/// struct TextureCompressor;
///
/// impl Process for TextureCompressor {
///     type Asset = Texture;
///     type Output = Texture;
///     type Error = std::io::Error;
///     
///     fn process(
///         &self,
///         input: LoadedAsset<Self::Asset>,
///     ) -> Result<Self::Output, Self::Error> {
///         // 压缩纹理
///         let compressed = compress_texture(&input)?;
///         Ok(compressed)
///     }
/// }
/// ```
pub trait Process: Send + Sync + 'static {
    /// Input asset type.
    type Asset: Asset;
    
    /// Output asset type.
    type Output: Asset;
    
    /// Error type.
    type Error: std::error::Error + Send + Sync + 'static;
    
    /// Process an asset.
    fn process(
        &self,
        input: LoadedAsset<Self::Asset>,
    ) -> Result<Self::Output, Self::Error>;
}
```

**AssetProcessor 使用示例**：

```rust
use bevy_asset::{AssetProcessor, Process, LoadedAsset};
use bevy_ecs::prelude::*;

// 定义纹理压缩处理器
#[derive(Default)]
struct TextureCompressor;

impl Process for TextureCompressor {
    type Asset = Texture;
    type Output = Texture;
    type Error = std::io::Error;
    
    fn process(
        &self,
        input: LoadedAsset<Self::Asset>,
    ) -> Result<Self::Output, Self::Error> {
        // 压缩纹理数据
        let compressed_data = compress_texture_data(&input.data)?;
        
        Ok(Texture {
            data: compressed_data,
            format: TextureFormat::Bc3UnormSrgb,  // 压缩格式
            ..input
        })
    }
}

// 定义音频编码处理器
#[derive(Default)]
struct AudioEncoder;

impl Process for AudioEncoder {
    type Asset = AudioSource;
    type Output = AudioSource;
    type Error = std::io::Error;
    
    fn process(
        &self,
        input: LoadedAsset<Self::Asset>,
    ) -> Result<Self::Output, Self::Error> {
        // 将 WAV 编码为 OGG
        let encoded_data = encode_to_ogg(&input.data)?;
        
        Ok(AudioSource {
            data: encoded_data,
            format: AudioFormat::Ogg,
            ..input
        })
    }
}

// 注册处理器
fn register_processors(app: &mut App) {
    app
        // 注册纹理压缩器为 PNG 的默认处理器
        .register_asset_processor("png", TextureCompressor::default())
        
        // 注册音频编码器为 WAV 的默认处理器
        .register_asset_processor("wav", AudioEncoder::default());
}

// 手动处理资源
async fn manual_processing(
    asset_processor: Res<AssetProcessor>,
) {
    // 处理单个资源
    asset_processor.process("textures/player.png").await?;
    
    // 处理所有资源
    asset_processor.process_all().await?;
}
```

**AssetTransformer 示例**：

```rust
use bevy_asset::{AssetTransformer, TransformContext};
use bevy_ecs::prelude::*;

// 定义纹理转换器
#[derive(Default)]
struct ResizeToPowerOfTwo;

impl AssetTransformer for ResizeToPowerOfTwo {
    type Asset = Texture;
    type Output = Texture;
    type Error = std::io::Error;
    
    async fn transform(
        &self,
        asset: Self::Asset,
        _transform_context: &mut TransformContext,
    ) -> Result<Self::Output, Self::Error> {
        // 调整大小到 2 的幂
        let new_width = next_power_of_two(asset.width);
        let new_height = next_power_of_two(asset.height);
        
        if new_width == asset.width && new_height == asset.height {
            return Ok(asset);
        }
        
        let resized_texture = resize_texture(&asset, new_width, new_height)?;
        
        Ok(resized_texture)
    }
}

// 使用 LoadTransformAndSave
fn setup_texture_processor(app: &mut App) {
    app.register_asset_processor(
        "png",
        LoadTransformAndSave::new(ResizeToPowerOfTwo::default()),
    );
}
```

---

### 7. Asset I/O 系统

**文件**: [`io/mod.rs`](file:///d:/work/ttc/bevy/crates/bevy_asset/src/io/mod.rs)

#### AssetReader 特质定义

```rust
/// A trait for reading assets from a source.
///
/// Bevy provides implementations for:
/// - File system
/// - Embedded assets
/// - HTTP/HTTPS
/// - Memory
///
/// You can implement `AssetReader` for custom sources.
pub trait AssetReader: Send + Sync + 'static {
    /// 读取资源
    async fn read(&self, path: &str) -> Result<Vec<u8>, AssetReaderError>;
    
    /// 获取资源元数据
    async fn metadata(&self, path: &str) -> Result<Metadata, AssetReaderError>;
    
    /// 列出目录内容
    async fn read_dir(&self, path: &str) -> Result<Vec<String>, AssetReaderError>;
}
```

**AssetSource 结构**：

```rust
/// A source of assets.
///
/// `AssetSource` combines an `AssetReader` and `AssetWriter`.
/// It can be used to load and save assets.
#[derive(Clone)]
pub struct AssetSource {
    reader: Arc<dyn AssetReader>,
    writer: Option<Arc<dyn AssetWriter>>,
    watcher: Option<Arc<dyn AssetWatcher>>,
}
```

**Asset I/O 实现示例**：

```rust
use bevy_asset::io::{AssetReader, AssetWriter, AssetSource};
use std::path::Path;

// 文件系统读取器
fn setup_file_source(app: &mut App) {
    app.add_asset_source(
        "assets",
        AssetSource::from_directory("path/to/assets"),
    );
}

// 嵌入资源读取器
fn setup_embedded_source(app: &mut App) {
    app.add_asset_source(
        "embedded",
        AssetSource::from_embedded(),
    );
}

// 网络资源读取器
fn setup_web_source(app: &mut App) {
    app.add_asset_source(
        "web",
        AssetSource::from_url("https://example.com/assets"),
    );
}

// 内存资源读取器
fn setup_memory_source(app: &mut App) {
    let mut memory = MemoryAssetReader::new();
    memory.add_file("textures/test.png", vec![1, 2, 3, 4]);
    
    app.add_asset_source(
        "memory",
        AssetSource::from_reader(memory),
    );
}

// 从多个源加载资源
fn load_from_multiple_sources(asset_server: Res<AssetServer>) {
    // 从文件系统加载
    let file_texture: Handle<Texture> = asset_server.load("assets://textures/player.png");
    
    // 从嵌入资源加载
    let embedded_texture: Handle<Texture> = asset_server.load("embedded://shaders/default.wgsl");
    
    // 从网络加载
    let web_texture: Handle<Texture> = asset_server.load("https://example.com/textures/icon.png");
}
```

**嵌入资源宏**：

```rust
use bevy_asset::embedded_asset;

fn setup_embedded_assets(app: &mut App) {
    // 嵌入单个资源
    embedded_asset!(app, "shaders/custom.wgsl");
    
    // 嵌入多个资源
    embedded_asset!(app, "textures/icon.png");
    embedded_asset!(app, "fonts/default.ttf");
    
    // 现在可以像普通资源一样加载
    // asset_server.load("shaders/custom.wgsl")
}
```

---

## 典型使用场景

### 1. 完整资源加载流程

```rust
use bevy_asset::{Asset, AssetServer, Assets, Handle, LoadState};
use bevy_ecs::prelude::*;
use std::collections::HashSet;

// 定义游戏状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum GameState {
    Loading,
    Playing,
}

// 定义加载资源
#[derive(Resource)]
struct LoadingAssets {
    textures: HashSet<Handle<Texture>>,
    meshes: HashSet<Handle<Mesh>>,
    scenes: HashSet<Handle<Scene>>,
}

// 开始加载
fn start_loading(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    // 加载纹理
    let mut textures = HashSet::new();
    textures.insert(asset_server.load("textures/player.png"));
    textures.insert(asset_server.load("textures/enemy.png"));
    textures.insert(asset_server.load("textures/background.png"));
    textures.insert(asset_server.load("textures/ui.png"));
    
    // 加载模型
    let mut meshes = HashSet::new();
    meshes.insert(asset_server.load("meshes/player.glb"));
    meshes.insert(asset_server.load("meshes/enemy.glb"));
    meshes.insert(asset_server.load("meshes/terrain.glb"));
    
    // 加载场景
    let mut scenes = HashSet::new();
    scenes.insert(asset_server.load("scenes/level1.glb"));
    
    // 存储到资源
    commands.insert_resource(LoadingAssets { textures, meshes, scenes });
    
    // 切换到加载状态
    commands.insert_resource(NextState(GameState::Loading));
}

// 检查加载进度
fn check_loading(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut loading_assets: ResMut<LoadingAssets>,
) {
    // 检查所有资源是否加载完成
    let all_textures_loaded = loading_assets
        .textures
        .iter()
        .all(|handle| asset_server.is_loaded_with_dependencies(handle.clone()));
    
    let all_meshes_loaded = loading_assets
        .meshes
        .iter()
        .all(|handle| asset_server.is_loaded_with_dependencies(handle.clone()));
    
    let all_scenes_loaded = loading_assets
        .scenes
        .iter()
        .all(|handle| asset_server.is_loaded_with_dependencies(handle.clone()));
    
    if all_textures_loaded && all_meshes_loaded && all_scenes_loaded {
        // 所有资源加载完成
        println!("All assets loaded!");
        
        // 存储已加载的资源句柄
        commands.insert_resource(LoadedAssets {
            textures: loading_assets.textures.clone(),
            meshes: loading_assets.meshes.clone(),
            scenes: loading_assets.scenes.clone(),
        });
        
        // 切换到游戏状态
        commands.insert_resource(NextState(GameState::Playing));
        
        // 清理加载资源
        commands.remove_resource::<LoadingAssets>();
    } else {
        // 打印加载进度
        let loaded_textures = loading_assets
            .textures
            .iter()
            .filter(|h| asset_server.is_loaded_with_dependencies(h.clone()))
            .count();
        
        println!("Loading: {}/{} textures", loaded_textures, loading_assets.textures.len());
    }
}

// 使用已加载的资源
fn spawn_player(
    mut commands: Commands,
    loaded_assets: Res<LoadedAssets>,
    asset_server: Res<AssetServer>,
) {
    // 获取玩家纹理
    let player_texture = loaded_assets
        .textures
        .iter()
        .find(|h| {
            let path = asset_server.get_path(h).unwrap();
            path.ends_with("player.png")
        })
        .unwrap()
        .clone();
    
    // 生成玩家
    commands.spawn((
        PbrBundle {
            mesh: asset_server.load("meshes/player.glb"),
            material: asset_server.load("materials/player.material"),
            ..default()
        },
        Player,
    ));
}

// 配置应用
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_state::<GameState>()
        .add_systems(Startup, start_loading)
        .add_systems(Update, check_loading.run_if(in_state(GameState::Loading)))
        .add_systems(OnEnter(GameState::Playing), spawn_player)
        .run();
}
```

### 2. 自定义资源类型和加载器

```rust
use bevy_asset::{Asset, AssetLoader, LoadContext, LoadedAsset};
use bevy_ecs::prelude::*;
use bevy_reflect::TypePath;
use serde::{Deserialize, Serialize};
use std::io::Cursor;

// 1. 定义资源类型
#[derive(Asset, TypePath, Debug, Clone)]
struct CustomLevel {
    name: String,
    width: u32,
    height: u32,
    tiles: Vec<u8>,
    #[dependency]
    tileset: Handle<Texture>,
    entities: Vec<EntityData>,
}

#[derive(Debug, Clone)]
struct EntityData {
    position: Vec3,
    rotation: Quat,
    entity_type: String,
}

// 2. 定义加载器设置
#[derive(Default, Serialize, Deserialize)]
struct CustomLevelLoaderSettings {
    scale: f32,
    flip_vertical: bool,
}

// 3. 定义加载器错误
#[derive(Debug, thiserror::Error)]
enum CustomLevelLoaderError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Invalid header")]
    InvalidHeader,
    
    #[error("Parse error: {0}")]
    Parse(String),
}

// 4. 实现资源加载器
#[derive(Default)]
struct CustomLevelLoader;

impl AssetLoader for CustomLevelLoader {
    type Asset = CustomLevel;
    type Settings = CustomLevelLoaderSettings;
    type Error = CustomLevelLoaderError;
    
    async fn load(
        &self,
        bytes: &[u8],
        load_context: &mut LoadContext,
        settings: &Self::Settings,
    ) -> Result<Self::Asset, Self::Error> {
        // 解析文件头
        let mut reader = Cursor::new(bytes);
        let mut header = [0u8; 4];
        reader.read_exact(&mut header)?;
        
        if &header != b"LVL2" {
            return Err(CustomLevelLoaderError::InvalidHeader);
        }
        
        // 读取元数据
        let name_length = reader.read_u32::<LittleEndian>()?;
        let mut name_bytes = vec![0u8; name_length as usize];
        reader.read_exact(&mut name_bytes)?;
        let name = String::from_utf8(name_bytes)
            .map_err(|e| CustomLevelLoaderError::Parse(e.to_string()))?;
        
        let width = reader.read_u32::<LittleEndian>()?;
        let height = reader.read_u32::<LittleEndian>()?;
        
        // 加载 tileset
        let tileset_path = format!("textures/levels/{}/tileset.png", name);
        let tileset_handle: Handle<Texture> = load_context.load(tileset_path);
        
        // 读取瓦片数据
        let tiles_count = reader.read_u32::<LittleEndian>()?;
        let mut tiles = vec![0u8; tiles_count as usize];
        reader.read_exact(&mut tiles)?;
        
        // 应用设置
        if settings.flip_vertical {
            tiles = flip_vertical(&tiles, width, height);
        }
        
        // 读取实体数据
        let entity_count = reader.read_u32::<LittleEndian>()?;
        let mut entities = Vec::with_capacity(entity_count as usize);
        
        for _ in 0..entity_count {
            let x = reader.read_f32::<LittleEndian>()? * settings.scale;
            let y = reader.read_f32::<LittleEndian>()? * settings.scale;
            let z = reader.read_f32::<LittleEndian>()? * settings.scale;
            
            let rx = reader.read_f32::<LittleEndian>()?;
            let ry = reader.read_f32::<LittleEndian>()?;
            let rz = reader.read_f32::<LittleEndian>()?;
            let rw = reader.read_f32::<LittleEndian>()?;
            
            let type_length = reader.read_u32::<LittleEndian>()?;
            let mut type_bytes = vec![0u8; type_length as usize];
            reader.read_exact(&mut type_bytes)?;
            let entity_type = String::from_utf8(type_bytes)
                .map_err(|e| CustomLevelLoaderError::Parse(e.to_string()))?;
            
            entities.push(EntityData {
                position: Vec3::new(x, y, z),
                rotation: Quat::from_xyzw(rx, ry, rz, rw),
                entity_type,
            });
        }
        
        Ok(CustomLevel {
            name,
            width: (width as f32 * settings.scale) as u32,
            height: (height as f32 * settings.scale) as u32,
            tiles,
            tileset: tileset_handle,
            entities,
        })
    }
    
    fn extensions(&self) -> &[&str] {
        &["lvl", "level"]
    }
}

// 5. 注册资源和加载器
fn register_custom_asset(app: &mut App) {
    app
        .init_asset::<CustomLevel>()
        .register_asset_loader(CustomLevelLoader::default());
}

// 6. 使用自定义资源
fn load_custom_level(asset_server: Res<AssetServer>) {
    let level_handle: Handle<CustomLevel> = asset_server.load("levels/level1.lvl");
    
    // 使用资源
    // ...
}
```

### 3. 资源热重载

```rust
use bevy_asset::{AssetEvent, AssetServer, Handle};
use bevy_ecs::prelude::*;

// 监听资源变化
fn listen_to_asset_changes(
    mut texture_events: EventReader<AssetEvent<Texture>>,
    mut material_events: EventReader<AssetEvent<StandardMaterial>>,
    mut shader_events: EventReader<AssetEvent<Shader>>,
    materials: Res<Assets<StandardMaterial>>,
) {
    // 监听纹理变化
    for event in texture_events.iter() {
        match event {
            AssetEvent::Modified { handle } => {
                println!("Texture modified: {:?}", handle);
            }
            AssetEvent::Removed { handle } => {
                println!("Texture removed: {:?}", handle);
            }
            _ => {}
        }
    }
    
    // 监听材质变化
    for event in material_events.iter() {
        if let AssetEvent::Modified { handle } = event {
            if let Some(material) = materials.get(handle) {
                println!("Material modified: albedo = {:?}", material.albedo);
            }
        }
    }
    
    // 监听着色器变化
    for event in shader_events.iter() {
        if let AssetEvent::Modified { handle } = event {
            println!("Shader modified: {:?}", handle);
            // 着色器会自动重新编译
        }
    }
}

// 热重载着色器
fn setup_hot_reload(mut commands: Commands, asset_server: Res<AssetServer>) {
    // 加载着色器
    let shader_handle: Handle<Shader> = asset_server.load("shaders/custom.wgsl");
    
    // 创建材质
    commands.spawn((
        PbrBundle {
            mesh: asset_server.load("meshes/cube.glb"),
            material: asset_server.load("materials/custom.material"),
            ..default()
        },
    ));
    
    // 修改 shaders/custom.wgsl 文件
    // 材质会自动更新
}

// 配置热重载
fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            // 启用热重载
            watch_for_changes_override: Some(true),
            ..default()
        }))
        .add_systems(Update, listen_to_asset_changes)
        .run();
}
```

---

## 资源处理流程

### 完整资源加载流程

```text
1. 用户调用 AssetServer::load("path/to/asset.ext")
   ↓
2. AssetServer 检查缓存
   ├─ 如果缓存存在:
   │  └─ 返回现有 Handle
   └─ 如果缓存不存在:
      └─ 继续
   ↓
3. AssetServer 确定资源类型
   ├─ 根据扩展名查找 AssetLoader
   └─ 创建 LoadContext
   ↓
4. AssetServer 读取资源字节
   ├─ 使用 AssetReader 读取
   ├─ 支持多种源（文件、嵌入、网络）
   └─ 处理 .meta 文件
   ↓
5. AssetLoader 解析字节
   ├─ 调用 AssetLoader::load()
   ├─ 加载依赖资源
   └─ 返回 Asset
   ↓
6. AssetServer 存储资源
   ├─ 添加到 Assets<T> 集合
   ├─ 触发 AssetEvent::Added
   └─ 返回 Handle
   ↓
7. 用户使用 Handle
   ├─ 通过 Assets<T>::get(handle) 访问
   ├─ 克隆 Handle（增加引用计数）
   └─ 丢弃 Handle（减少引用计数）
   ↓
8. 引用计数为 0
   ├─ 资源从 Assets<T> 中移除
   ├─ 触发 AssetEvent::Removed
   └─ 资源从内存中释放
```

### 资源处理流程

```text
1. AssetProcessor 检测资源变化
   ├─ AssetWatcher 监控文件系统
   ├─ 检测到新文件或修改的文件
   └─ 触发处理流程
   ↓
2. AssetProcessor 读取原始资源
   ├─ 使用 AssetReader 读取
   └─ 计算资源哈希
   ↓
3. AssetProcessor 检查缓存
   ├─ 如果缓存存在且哈希匹配:
   │  └─ 跳过处理
   └─ 如果缓存不存在或哈希不匹配:
      └─ 继续
↓
4. AssetProcessor 应用处理器
   ├─ 查找匹配的 Process
   ├─ 调用 Process::process()
   └─ 转换资源
   ↓
5. AssetProcessor 保存处理结果
   ├─ 使用 AssetWriter 写入
   ├─ 生成 .meta 文件
   └─ 更新缓存
   ↓
6. AssetProcessor 记录事务
   ├─ 写入事务日志
   ├─ 支持崩溃恢复
   └─ 标记处理完成
   ↓
7. AssetServer 加载处理后的资源
   ├─ 从处理后的路径加载
   ├─ 使用处理后的资源
   └─ 触发 AssetEvent::Modified
```

### 热重载流程

```text
1. AssetWatcher 监控文件系统
   ├─ 使用 inotify (Linux) 或 ReadDirectoryChangesW (Windows)
   ├─ 检测文件创建、修改、删除
   └─ 触发事件
   ↓
2. AssetServer 处理变化事件
   ├─ 识别变化的资源
   ├─ 检查是否需要重新加载
   └─ 取消正在进行的加载
   ↓
3. AssetServer 重新加载资源
   ├─ 重新读取字节
   ├─ 重新解析资源
   ├─ 重新加载依赖
   └─ 更新 Assets<T> 集合
   ↓
4. AssetServer 触发事件
   ├─ 触发 AssetEvent::Modified
   ├─ 所有使用该资源的实体自动更新
   └─ 材质、着色器自动重新编译
   ↓
5. 用户看到变化
   ├─ 游戏中的资源立即更新
   ├─ 无需重启游戏
   └─ 支持实时迭代
```

---

## 性能优化建议

### 1. 减少内存占用

```rust
// 使用弱句柄缓存
#[derive(Resource)]
struct AssetCache {
    textures: HashMap<String, WeakHandle<Texture>>,
    meshes: HashMap<String, WeakHandle<Mesh>>,
}

fn cache_asset(
    mut cache: ResMut<AssetCache>,
    asset_server: Res<AssetServer>,
) {
    let handle: Handle<Texture> = asset_server.load("textures/player.png");
    
    // 存储弱句柄（不增加引用计数）
    cache.textures.insert(
        "player".to_string(),
        handle.downgrade(),
    );
}

fn get_cached_asset(
    cache: Res<AssetCache>,
    mut textures: ResMut<Assets<Texture>>,
) -> Option<Handle<Texture>> {
    if let Some(weak_handle) = cache.textures.get("player") {
        // 升级弱句柄（如果资源仍在内存中）
        weak_handle.upgrade()
    } else {
        None
    }
}
```

### 2. 按需加载

```rust
// 按关卡加载资源
#[derive(Resource)]
struct LevelAssets {
    textures: Vec<Handle<Texture>>,
    meshes: Vec<Handle<Mesh>>,
    scenes: Vec<Handle<Scene>>,
}

fn load_level_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    level: Res<CurrentLevel>,
) {
    let mut textures = Vec::new();
    let mut meshes = Vec::new();
    let mut scenes = Vec::new();
    
    // 加载当前关卡需要的资源
    for texture_path in &level.texture_paths {
        textures.push(asset_server.load(texture_path));
    }
    
    for mesh_path in &level.mesh_paths {
        meshes.push(asset_server.load(mesh_path));
    }
    
    commands.insert_resource(LevelAssets { textures, meshes, scenes });
}

fn unload_level_assets(
    mut commands: Commands,
) {
    // 移除 LevelAssets 资源
    // 所有强句柄被丢弃
    // 引用计数减少
    // 未被其他关卡使用的资源被卸载
    commands.remove_resource::<LevelAssets>();
}
```

### 3. 异步加载

```rust
// 异步加载资源
async fn async_loading(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    // 并行加载多个资源
    let texture_future = asset_server.load_async("textures/large.png");
    let mesh_future = asset_server.load_async("meshes/complex.glb");
    let scene_future = asset_server.load_async("scenes/huge.glb");
    
    // 等待所有加载完成
    let (texture_handle, mesh_handle, scene_handle) = futures::join!(
        texture_future,
        mesh_future,
        scene_future,
    );
    
    // 使用已加载的资源
    commands.spawn((
        PbrBundle {
            mesh: mesh_handle,
            ..default()
        },
    ));
}
```

### 4. 资源处理优化

```rust
// 仅处理变化的资源
fn setup_asset_processor(app: &mut App) {
    app
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            mode: AssetMode::Processed,
            ..default()
        }))
        .register_asset_processor(
            "png",
            LoadTransformAndSave::new(TextureCompressor::default()),
        );
    
    // AssetProcessor 会自动：
    // - 仅处理新的或修改的资源
    // - 缓存处理结果
    // - 支持增量处理
}
```

---

## 文件结构

```
src/
├── server/                    # 资源服务器
│   ├── mod.rs               # AssetServer 实现
│   ├── info.rs              # 资源信息管理
│   └── loaders.rs           # 加载器管理
├── io/                       # 资源 I/O
│   ├── embedded/            # 嵌入资源
│   │   ├── mod.rs
│   │   └── embedded_watcher.rs
│   ├── file/                # 文件系统
│   │   ├── mod.rs
│   │   ├── file_asset.rs
│   │   ├── file_watcher.rs
│   │   └── sync_file_asset.rs
│   ├── mod.rs               # AssetReader/AssetWriter
│   ├── source.rs            # AssetSource
│   ├── memory.rs            # 内存资源
│   ├── gated.rs             # 门控资源
│   ├── processor_gated.rs   # 处理器门控
│   ├── android.rs           # Android 支持
│   ├── wasm.rs              # WASM 支持
│   └── web.rs               # Web 支持
├── processor/                # 资源处理器
│   ├── mod.rs               # AssetProcessor 实现
│   ├── process.rs           # Process 特质
│   ├── log.rs               # 事务日志
│   └── tests.rs             # 测试
├── assets.rs                # Assets<T> 集合
├── handle.rs                # Handle<T> 句柄
├── loader.rs                # AssetLoader 加载器
├── loader_builders.rs       # 加载器构建器
├── event.rs                 # AssetEvent 事件
├── id.rs                    # AssetId 资源 ID
├── path.rs                  # AssetPath 资源路径
├── meta.rs                  # AssetMeta 资源元数据
├── saver.rs                 # AssetSaver 资源保存器
├── transformer.rs           # AssetTransformer 资源转换器
├── reflect.rs               # 反射支持
├── render_asset.rs          # 渲染资源
├── folder.rs                # 文件夹资源
├── asset_changed.rs         # AssetChanged 查询过滤器
├── direct_access_ext.rs     # 直接访问扩展
└── lib.rs                   # 主入口和 AssetPlugin
```

---

## 常见问题

### 1. 资源加载失败

**可能原因**：
- 文件路径错误
- 文件格式不支持
- 缺少依赖资源
- 权限不足

**解决方法**：
```rust
fn check_asset_path(asset_server: Res<AssetServer>) {
    // 使用正确的路径格式
    let handle: Handle<Texture> = asset_server.load("textures/player.png");
    
    // 检查加载状态
    match asset_server.load_state(handle.clone()) {
        LoadState::Failed => {
            println!("Failed to load asset");
        }
        _ => {}
    }
}
```

### 2. 资源突然消失

**可能原因**：
- 所有强句柄被丢弃
- 引用计数变为 0
- 资源被从内存中移除

**解决方法**：
```rust
// 保留强句柄
#[derive(Resource)]
struct PersistentAssets {
    textures: Vec<Handle<Texture>>,
    meshes: Vec<Handle<Mesh>>,
}

fn keep_assets_loaded(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let handle: Handle<Texture> = asset_server.load("textures/player.png");
    
    // 存储到资源中（永远保留）
    commands.insert_resource(PersistentAssets {
        textures: vec![handle],
        meshes: vec![],
    });
}
```

### 3. 热重载不工作

**可能原因**：
- 未启用 file_watcher 特性
- 未启用 AssetPlugin 的 watch_for_changes
- 文件系统监控失败

**解决方法**：
```rust
fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            watch_for_changes_override: Some(true),
            ..default()
        }))
        .run();
}
```

**Cargo.toml**：
```toml
[dependencies]
bevy = { version = "0.14", features = ["file_watcher"] }
```

### 4. 资源处理缓慢

**可能原因**：
- 资源处理需要大量计算
- 未使用缓存
- 每次都重新处理所有资源

**解决方法**：
```rust
fn setup_processing(app: &mut App) {
    app
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            mode: AssetMode::Processed,
            ..default()
        }))
        .register_asset_processor(
            "png",
            LoadTransformAndSave::new(TextureCompressor::default()),
        );
    
    // AssetProcessor 会自动：
    // - 缓存处理结果
    // - 仅处理变化的资源
    // - 使用事务日志
}
```

---

## 总结

`bevy_asset` 是一个**功能完整、高性能、可扩展的资源管理系统**，具有以下优势：

**核心优势**：
1. **异步加载**：非阻塞式资源加载，支持后台任务
2. **类型安全**：强类型资源系统，编译时类型检查
3. **自动管理**：引用计数自动资源生命周期管理
4. **热重载**：开发时自动检测并重新加载修改的资源
5. **资源处理**：自动资源转换、压缩、优化
6. **多源支持**：文件系统、嵌入资源、网络资源
7. **依赖管理**：自动处理资源之间的依赖关系

**适用场景**：
- 3D 游戏（需要加载大量纹理、模型、场景）
- 2D 游戏（需要加载大量精灵图、动画）
- 可视化（需要加载大量数据文件）
- VR/AR（需要加载高质量资源）
- 编辑器（需要支持资源热重载）

**学习资源**：
- [Bevy Asset 文档](https://docs.rs/bevy/latest/bevy/asset/index.html)
- [Bevy 示例](https://github.com/bevyengine/bevy/tree/latest/examples/asset)
- [Rust 异步编程](https://rust-lang.github.io/async-book/)
- [游戏资源管理最佳实践](https://www.gdcvault.com/)

---

**注意**：`bevy_asset` 是 Bevy 引擎的核心模块，与 `bevy_ecs`、`bevy_render`、`bevy_pbr` 等模块紧密配合。合理使用资源管理系统可以显著提升游戏的性能和开发效率。