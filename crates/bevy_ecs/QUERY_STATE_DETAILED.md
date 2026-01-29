# QueryState 深度解析

**基于 Bevy ECS 0.19.0-dev 版本**

## 目录

1. [什么是 QueryState](#什么是-querystate)
2. [核心结构](#核心结构)
3. [工作原理](#工作原理)
4. [密集 vs 稀疏查询](#密集-vs-稀疏查询)
5. [性能优化](#性能优化)
6. [实际使用示例](#实际使用示例)
7. [QueryState 与 Query 的关系](#querystate-与-query-的关系)
8. [0.19 版本新增特性](#019-版本新增特性)

---

## 什么是 QueryState

`QueryState` 是 Bevy ECS 中用于**缓存查询元数据**的核心结构。它存储了关于查询如何访问 World 的关键信息，以便在系统运行时能够快速执行查询。

### 核心作用

- **缓存匹配信息**：存储哪些表或原型与查询匹配
- **存储访问信息**：用于并行执行时的冲突检测
- **缓存查询状态**：存储如何从存储中获取数据的信息
- **优化迭代性能**：通过双重存储提高查询速度

### 生命周期

```
系统初始化
    ↓
创建 QueryState（缓存元数据）
    ↓
系统运行（多次）
    ├─ 使用缓存的 QueryState
    ├─ 自动检查原型变化
    └─ 仅在必要时更新匹配信息
    ↓
系统销毁
```

---

## 核心结构

```rust
/// Provides scoped access to a [`World`] state according to a given [`QueryData`] and [`QueryFilter`].
///
/// This data is cached between system runs, and is used to:
/// - store metadata about which [`Table`] or [`Archetype`] are matched by the query.
/// - cache the [`State`] needed to compute the [`Fetch`] struct used to retrieve data
/// - build iterators that can iterate over the query results
///
/// [`State`]: crate::query::world_query::WorldQuery::State
/// [`Fetch`]: crate::query::world_query::WorldQuery::Fetch
/// [`Table`]: crate::storage::Table
#[repr(C)]
// SAFETY NOTE:
// Do not add any new fields that use the `D` or `F` generic parameters as this may
// make `QueryState::as_transmuted_state` unsound if not done with care.
pub struct QueryState<D: QueryData, F: QueryFilter = ()> {
    world_id: WorldId,
    pub(crate) archetype_generation: ArchetypeGeneration,
    /// Metadata about the [`Table`](crate::storage::Table)s matched by this query.
    pub(crate) matched_tables: FixedBitSet,
    /// Metadata about the [`Archetype`]s matched by this query.
    pub(crate) matched_archetypes: FixedBitSet,
    /// [`FilteredAccess`] computed by combining the `D` and `F` access. Used to check which other queries
    /// this query can run in parallel with.
    pub(crate) component_access: FilteredAccess,
    // NOTE: we maintain both a bitset and a vec because iterating the vec is faster
    pub(super) matched_storage_ids: Vec<StorageId>,
    // Represents whether this query iteration is dense or not. When this is true
    // `matched_storage_ids` stores `TableId`s, otherwise it stores `ArchetypeId`s.
    pub(super) is_dense: bool,
    pub(crate) fetch_state: D::State,
    pub(crate) filter_state: F::State,
    #[cfg(feature = "trace")]
    par_iter_span: Span,
}
```

### 字段详解

| 字段 | 类型 | 作用 |
|------|------|------|
| `world_id` | `WorldId` | 标识这个 QueryState 属于哪个 World |
| `archetype_generation` | `ArchetypeGeneration` | 跟踪世界的原型生成版本，用于判断是否需要更新匹配信息 |
| `matched_tables` | `FixedBitSet` | 存储哪些表与查询匹配（用于快速查找） |
| `matched_archetypes` | `FixedBitSet` | 存储哪些原型与查询匹配（用于快速查找） |
| `component_access` | `FilteredAccess` | 存储查询需要访问的组件，用于并行执行时的冲突检测 |
| `matched_storage_ids` | `Vec<StorageId>` | 存储匹配的存储 ID（表或原型），用于快速迭代 |
| `is_dense` | `bool` | 标识查询是密集的还是稀疏的 |
| `fetch_state` | `D::State` | 缓存 QueryData 的状态，包含如何从存储中获取数据的信息 |
| `filter_state` | `F::State` | 缓存 QueryFilter 的状态，包含如何过滤实体的信息 |
| `par_iter_span` | `Span` | （可选）用于跟踪并行迭代的性能（需要 `trace` 特性） |

### StorageId 联合类型

```rust
/// An ID for either a table or an archetype. Used for Query iteration.
///
/// Query iteration is exclusively dense (over tables) or archetypal (over archetypes) based on whether
/// the query filters are dense or not. This is represented by the [`QueryState::is_dense`] field.
///
/// Note that `D::IS_DENSE` and `F::IS_DENSE` have no relationship with `QueryState::is_dense` and
/// any combination of their values can happen.
///
/// This is a union instead of an enum as the usage is determined at compile time, as all [`StorageId`]s for
/// a [`QueryState`] will be all [`TableId`]s or all [`ArchetypeId`]s, and not a mixture of both. This
/// removes the need for discriminator to minimize memory usage and branching during iteration, but requires
/// a safety invariant be verified when disambiguating them.
///
/// # Safety
/// Must be initialized and accessed as a [`TableId`], if both generic parameters to the query are dense.
/// Must be initialized and accessed as an [`ArchetypeId`] otherwise.
#[derive(Clone, Copy)]
pub(super) union StorageId {
    pub(super) table_id: TableId,
    pub(super) archetype_id: ArchetypeId,
}
```

---

## 工作原理

### 创建阶段

```rust
// 1. 创建未初始化的 QueryState
fn new_uninitialized(world: &mut World) -> Self {
    let fetch_state = D::init_state(world);
    let filter_state = F::init_state(world);
    Self::from_states_uninitialized(world, fetch_state, filter_state)
}

// 2. 初始化状态（计算组件访问信息）
fn from_states_uninitialized(
    world: &World,
    fetch_state: D::State,
    filter_state: F::State,
) -> Self {
    let mut component_access = FilteredAccess::default();
    D::update_component_access(&fetch_state, &mut component_access);
    
    let mut filter_component_access = FilteredAccess::default();
    F::update_component_access(&filter_state, &mut filter_component_access);
    
    component_access.extend(&filter_component_access);
    
    // 考虑默认查询过滤器
    let mut is_dense = D::IS_DENSE && F::IS_DENSE;
    
    if let Some(default_filters) = world.get_resource::<DefaultQueryFilters>() {
        default_filters.modify_access(&mut component_access);
        is_dense &= default_filters.is_dense(world.components());
    }
    
    Self {
        world_id: world.id(),
        archetype_generation: ArchetypeGeneration::initial(),
        matched_storage_ids: Vec::new(),
        is_dense,
        fetch_state,
        filter_state,
        component_access,
        matched_tables: Default::default(),
        matched_archetypes: Default::default(),
        #[cfg(feature = "trace")]
        par_iter_span: tracing::info_span!(
            "par_for_each",
            query = core::any::type_name::<D>(),
            filter = core::any::type_name::<F>(),
        ),
    }
}

// 3. 更新原型信息（找到匹配的表/原型）
fn update_archetypes(&mut self, world: &World) {
    // 检查原型是否有变化
    if world.archetypes().generation() == self.archetype_generation {
        return; // 没有变化，直接返回
    }
    
    // 重新计算匹配的表/原型
    self.matched_tables.clear();
    self.matched_archetypes.clear();
    self.matched_storage_ids.clear();
    
    for archetype in world.archetypes().iter() {
        if self.matches_archetype(archetype) {
            if self.is_dense {
                // 密集查询：存储表 ID
                for table_id in archetype.table_ids() {
                    self.matched_tables.insert(table_id.index());
                    self.matched_storage_ids.push(StorageId { table_id });
                }
            } else {
                // 稀疏查询：存储原型 ID
                self.matched_archetypes.insert(archetype.id().index());
                self.matched_storage_ids.push(StorageId { archetype_id: archetype.id() });
            }
        }
    }
    
    self.archetype_generation = world.archetypes().generation();
}
```

### 迭代阶段

```rust
// 1. 创建迭代器
fn iter<'w, 's>(&'s mut self, world: &'w World) -> QueryIter<'w, 's, D::ReadOnly, F> {
    self.query(world).into_iter()
}

// 2. 创建 Query（内部使用）
fn query<'w, 's>(&'s mut self, world: &'w World) -> Query<'w, 's, D::ReadOnly, F> {
    self.update_archetypes(world); // 确保原型信息是最新的
    self.query_manual(world)
}

// 3. 手动创建 Query（不更新原型）
fn query_manual<'w, 's>(&'s self, world: &'w World) -> Query<'w, 's, D::ReadOnly, F> {
    // 使用缓存的 fetch_state 和 filter_state 创建 Fetch 结构
    let fetch = unsafe { D::ReadOnly::fetch(&self.fetch_state, world) };
    let filter_fetch = unsafe { F::fetch(&self.filter_state, world) };
    
    Query {
        world: world.as_unsafe_world_cell(),
        state: self,
        fetch,
        filter_fetch,
        last_run: Tick::new(0),
        this_run: world.last_change_tick(),
    }
}
```

---

## 密集 vs 稀疏查询

### 密集查询（Dense Query）

```rust
// 只访问表存储（Table Storage）
// 数据在内存中是连续的，缓存命中率高
// 适合访问大量实体的组件
// 示例：Query<&Transform>

if self.is_dense {
    // 存储表 ID
    for table_id in archetype.table_ids() {
        self.matched_tables.insert(table_id.index());
        self.matched_storage_ids.push(StorageId { table_id });
    }
}
```

### 稀疏查询（Sparse Query）

```rust
// 需要访问原型存储（Archetype Storage）
// 数据在内存中可能不连续
// 适合访问少量实体或有复杂过滤器的查询
// 示例：Query<&Transform, Added<Transform>>

if !self.is_dense {
    // 存储原型 ID
    self.matched_archetypes.insert(archetype.id().index());
    self.matched_storage_ids.push(StorageId { archetype_id: archetype.id() });
}
```

### 对比表

| 特性 | 密集查询 | 稀疏查询 |
|------|----------|----------|
| 存储类型 | 表（Table） | 原型（Archetype） |
| 内存布局 | 连续 | 可能不连续 |
| 缓存命中率 | 高 | 低 |
| 适合场景 | 访问大量实体 | 访问少量实体或有复杂过滤器 |
| 示例 | `Query<&Transform>` | `Query<&Transform, Added<Transform>>` |
| 迭代速度 | 快 | 慢 |
| 内存占用 | 低 | 高 |

### 如何判断查询类型

```rust
// 查询的密集性由以下因素决定：
let is_dense = D::IS_DENSE && F::IS_DENSE;

// 如果存在默认查询过滤器，还需要考虑：
if let Some(default_filters) = world.get_resource::<DefaultQueryFilters>() {
    is_dense &= default_filters.is_dense(world.components());
}

// D::IS_DENSE 和 F::IS_DENSE 是编译时常量
// 由 QueryData 和 QueryFilter 的实现决定
```

---

## 性能优化

### 1. 双重存储优化

```rust
// QueryState 同时维护 bitset 和向量
matched_tables: FixedBitSet,      // 用于快速查找
matched_storage_ids: Vec<StorageId>, // 用于快速迭代

// 这样做的好处：
// - bitset 适合快速判断某个表是否匹配（O(1) 时间复杂度）
// - 向量适合快速遍历所有匹配的表（O(n) 时间复杂度，缓存友好）
// - 避免了在迭代时需要遍历整个 bitset

// 对比：如果只使用 bitset
for i in 0..bitset.len() {
    if bitset.contains(i) {  // 可能会访问不连续的内存
        // 处理
    }
}

// 使用向量
for storage_id in &self.matched_storage_ids {
    // 连续内存访问，缓存命中率高
}
```

### 2. 原型生成跟踪

```rust
// 只在原型变化时更新匹配信息
if world.archetypes().generation() == self.archetype_generation {
    return; // 跳过更新
}

// 这样可以避免在每次系统运行时都重新计算匹配信息
// 只有在实体被添加/删除/修改组件时才需要更新

// 性能提升：
// - 系统运行 1000 次，只有 10 次原型变化
// - 避免了 990 次不必要的匹配计算
```

### 3. 零成本转换

```rust
// 可以在运行时将 QueryState 转换为只读版本
pub fn as_readonly(&self) -> &QueryState<D::ReadOnly, F> {
    unsafe { self.as_transmuted_state::<D::ReadOnly, F>() }
}

// 这是一个零成本操作（只是指针转换）
// 因为只读查询的状态与原查询的状态是兼容的
unsafe fn as_transmuted_state<NewD, NewF>(&self) -> &QueryState<NewD, NewF> {
    &*ptr::from_ref(self).cast::<QueryState<NewD, NewF>>()
}

// 为什么这是安全的？
// - ReadOnlyQueryData 保证了 NewD 是 D 的只读版本
// - NewD::State == D::State（状态类型相同）
// - NewD 的访问是 D 的访问的子集
// - 因此可以安全地转换指针
```

### 4. 并行迭代优化

```rust
// 0.19 版本新增：并行迭代时的批处理优化
#[cfg(all(not(target_arch = "wasm32"), feature = "multi_threaded"))]
pub fn par_fold<T, FN, INIT>(
    &mut self,
    world: &World,
    init_accum: INIT,
    mut func: FN,
) -> T
where
    FN: Fn(T, D::Item<'_, '_>) -> T + Send + Sync + Clone,
    INIT: Fn() -> T + Sync + Send + Clone,
    D: ReadOnlyQueryData,
{
    use arrayvec::ArrayVec;
    
    bevy_tasks::ComputeTaskPool::get().scope(|scope| {
        let tables = unsafe { &world.storages().tables };
        let archetypes = world.archetypes();
        let mut batch_queue = ArrayVec::new();
        let mut queue_entity_count = 0;
        
        // 将小的存储合并为一个任务
        let submit_batch_queue = |queue: &mut ArrayVec<StorageId, 128>| {
            if queue.is_empty() {
                return;
            }
            let queue = core::mem::take(queue);
            let mut func = func.clone();
            let init_accum = init_accum.clone();
            scope.spawn(async move {
                #[cfg(feature = "trace")]
                let _span = self.par_iter_span.enter();
                let mut iter = self
                    .query_unchecked_manual_with_ticks(world, last_run, this_run)
                    .into_iter();
                let mut accum = init_accum();
                for storage_id in queue {
                    accum = iter.fold_over_storage_range(accum, &mut func, storage_id, None);
                }
            });
        };
        
        // 大的存储单独作为一个任务
        let submit_single = |count, storage_id: StorageId| {
            // ...
        };
        
        // 遍历所有存储，根据大小决定如何提交
        for storage_id in &self.matched_storage_ids {
            let count = if self.is_dense {
                unsafe { tables.get_unchecked(storage_id.table_id).len() }
            } else {
                unsafe { archetypes.get_unchecked(storage_id.archetype_id).len() }
            };
            
            if count > batch_size {
                submit_batch_queue(&mut batch_queue);
                submit_single(count, storage_id);
            } else {
                batch_queue.push(storage_id);
                queue_entity_count += count;
                if queue_entity_count > batch_size {
                    submit_batch_queue(&mut batch_queue);
                }
            }
        }
        
        submit_batch_queue(&mut batch_queue);
    });
}
```

---

## 实际使用示例

### 示例 1：基本使用

```rust
#[derive(Component)]
struct Position { x: f32, y: f32 }

#[derive(Component)]
struct Velocity { x: f32, y: f32 }

// 在系统中使用 QueryState
fn move_system(mut query: Query<(&mut Position, &Velocity)>) {
    // QueryState 被自动创建和缓存
    for (mut position, velocity) in &mut query {
        position.x += velocity.x;
        position.y += velocity.y;
    }
}

// 内部工作流程：
// 1. 第一次运行：创建 QueryState 并缓存匹配信息
// 2. 后续运行：使用缓存的 QueryState，只在原型变化时更新
```

### 示例 2：手动创建 QueryState

```rust
// 如果你需要在系统外部使用查询
let mut world = World::new();
world.spawn((Position { x: 0.0, y: 0.0 }, Velocity { x: 1.0, y: 1.0 }));

// 创建 QueryState
let mut query_state = QueryState::<(&Position, &Velocity)>::new(&mut world);

// 使用 QueryState 进行查询
for (position, velocity) in query_state.iter(&world) {
    println!("Position: ({}, {}), Velocity: ({}, {})", 
             position.x, position.y, velocity.x, velocity.y);
}
```

### 示例 3：并行迭代

```rust
// 使用 QueryState 进行并行查询
fn parallel_system(mut query: Query<&Transform>) {
    query.par_iter().for_each(|transform| {
        // 在多个线程中并行处理
        // QueryState 确保线程安全
    });
}

// 并行折叠（0.19 版本新增）
fn parallel_sum_system(query: Query<&Health>) {
    let total_health: i32 = query.par_fold(
        || 0,
        |sum, health| sum + health.0,
    );
    
    println!("Total health: {}", total_health);
}
```

### 示例 4：动态查询

```rust
// 使用 QueryBuilder 创建动态查询
let mut world = World::new();

let query = QueryBuilder::<&Position>::new(&world)
    .filter::<With<Velocity>>()
    .build();

// QueryBuilder 会创建一个 QueryState
for position in query.iter(&world) {
    // ...
}
```

### 示例 5：多实体查询

```rust
// 0.19 版本新增：get_many 和 get_many_mut 方法
fn multi_entity_system(query: Query<&Position>, entities: Vec<Entity>) {
    // 同时获取多个实体的组件
    let positions = query.get_many(&entities[0..3]);
    
    if let Ok([pos1, pos2, pos3]) = positions {
        println!("Positions: {:?}, {:?}, {:?}", pos1, pos2, pos3);
    }
}

// 可变版本
fn multi_entity_mut_system(mut query: Query<&mut Position>, entities: [Entity; 3]) {
    let mut positions = query.get_many_mut(entities).unwrap();
    
    for mut position in &mut positions {
        position.x += 1.0;
    }
}
```

### 示例 6：单实体查询

```rust
// 使用 single() 方法查询单个实体
fn camera_system(query: Query<&Camera>) {
    if let Ok(camera) = query.single() {
        // 处理单个相机
    }
}

// 使用 get() 方法查询特定实体
fn specific_entity_system(query: Query<&Position>, entity: Entity) {
    if let Ok(position) = query.get(entity) {
        // 处理特定实体的位置
    }
}
```

### 示例 7：只读转换

```rust
// 将可变查询转换为只读查询
fn read_only_system(query: Query<&mut Position>) {
    // 转换为只读版本（零成本）
    let readonly_query = query.as_readonly();
    
    // 可以同时使用多个只读迭代器
    for position in readonly_query.iter(&world) {
        // ...
    }
    
    for position in readonly_query.iter(&world) {
        // ...
    }
}
```

---

## QueryState 与 Query 的关系

```rust
// Query 是 QueryState 的临时包装器
pub struct Query<'w, 's, D: QueryData, F: QueryFilter = ()> {
    world: UnsafeWorldCell<'w>,
    state: &'s QueryState<D, F>, // 引用 QueryState
    fetch: D::Fetch<'w>,         // 从 QueryState 的 fetch_state 创建
    filter_fetch: F::Fetch<'w>,  // 从 QueryState 的 filter_state 创建
    last_run: Tick,
    this_run: Tick,
}

// Query 的生命周期是 'w（World 的生命周期）
// QueryState 的生命周期是 's（系统的生命周期）
// 这意味着 QueryState 可以在多个系统运行之间缓存
```

### 关系图

```
┌─────────────────────────────────────────────────────────┐
│                    World                                │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐     │
│  │  Archetype  │  │  Archetype  │  │  Archetype  │     │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘     │
│         │                │                │            │
│  ┌──────▼──────┐  ┌──────▼──────┐  ┌──────▼──────┐     │
│  │    Table    │  │    Table    │  │    Table    │     │
│  └─────────────┘  └─────────────┘  └─────────────┘     │
└─────────────────────────────────────────────────────────┘
                              │
                              │
┌─────────────────────────────▼─────────────────────────────┐
│                    QueryState                              │
│  ┌──────────────────────────────────────────────────┐    │
│  │  matched_tables: FixedBitSet                    │    │
│  │  matched_archetypes: FixedBitSet                │    │
│  │  matched_storage_ids: Vec<StorageId>            │    │
│  │  fetch_state: D::State                          │    │
│  │  filter_state: F::State                         │    │
│  └──────────────────────────────────────────────────┘    │
└───────────────────────────────────────────────────────────┘
                              │
                              │
┌─────────────────────────────▼─────────────────────────────┐
│                      Query                                │
│  ┌──────────────────────────────────────────────────┐    │
│  │  state: &QueryState<D, F>                        │    │
│  │  fetch: D::Fetch<'w>                             │    │
│  │  filter_fetch: F::Fetch<'w>                      │    │
│  └──────────────────────────────────────────────────┘    │
└───────────────────────────────────────────────────────────┘
                              │
                              │
┌─────────────────────────────▼─────────────────────────────┐
│                    Iterator                               │
│  ┌──────────────────────────────────────────────────┐    │
│  │  QueryIter / QueryParIter / QueryManyIter        │    │
│  └──────────────────────────────────────────────────┘    │
└───────────────────────────────────────────────────────────┘
```

### 对比表

| 特性 | QueryState | Query |
|------|------------|-------|
| 生命周期 | 长（在系统运行之间缓存） | 短（只在系统运行时创建） |
| 存储内容 | 元数据（匹配信息、访问信息、状态） | 实际数据访问器（Fetch 结构） |
| 作用 | 缓存查询信息，优化查询性能 | 临时包装器，提供查询接口 |
| 创建时机 | 系统初始化时 | 系统运行时 |
| 内存占用 | 较大（存储大量元数据） | 较小（只存储引用和 Fetch 结构） |
| 线程安全 | 是（只读） | 否（可变引用） |

---

## 0.19 版本新增特性

### 1. 并行折叠（Parallel Fold）

```rust
// 新增方法：par_fold、par_fold_init
pub fn par_fold<T, FN, INIT>(
    &mut self,
    world: &World,
    init_accum: INIT,
    mut func: FN,
) -> T
where
    FN: Fn(T, D::Item<'_, '_>) -> T + Send + Sync + Clone,
    INIT: Fn() -> T + Sync + Send + Clone,
    D: ReadOnlyQueryData,
{
    // 实现细节：
    // - 使用批处理优化
    // - 小的存储合并为一个任务
    // - 大的存储单独作为一个任务
    // - 自动平衡负载
}
```

### 2. 多实体查询

```rust
// 新增方法：get_many、get_many_mut、get_many_unique、get_many_unique_mut
pub fn get_many<'w, const N: usize>(
    &mut self,
    world: &'w World,
    entities: [Entity; N],
) -> Result<[ROQueryItem<'w, '_, D>; N], QueryEntityError> {
    self.query(world).get_many_inner(entities)
}

pub fn get_many_mut<'w, const N: usize>(
    &mut self,
    world: &'w mut World,
    entities: [Entity; N],
) -> Result<[QueryItem<'w, '_, D>; N], QueryEntityError> {
    self.query_mut(world).get_many_inner(entities)
}
```

### 3. 跟踪支持

```rust
// 新增字段：par_iter_span（可选）
#[cfg(feature = "trace")]
par_iter_span: Span,

// 使用方式：
// 在 Cargo.toml 中添加 feature = "trace"
// 然后可以使用 tracing 库跟踪并行迭代的性能
```

### 4. 安全改进

```rust
// 新增 SAFETY NOTE 注释
#[repr(C)]
// SAFETY NOTE:
// Do not add any new fields that use the `D` or `F` generic parameters as this may
// make `QueryState::as_transmuted_state` unsound if not done with care.
pub struct QueryState<D: QueryData, F: QueryFilter = ()> {
    // ...
}

// 提醒开发者：
// - 不要添加使用 D 或 F 泛型参数的字段
// - 否则会破坏 as_transmuted_state 的安全性
// - 因为 transmute 操作要求新旧类型的内存布局完全相同
```

---

## 最佳实践

### 1. 避免在系统外部创建 QueryState

```rust
// 不好的做法：
fn my_function(world: &World) {
    let mut query = QueryState::<&Position>::new(world); // 每次都创建新的 QueryState
    for position in query.iter(world) {
        // ...
    }
}

// 好的做法：
struct MySystem {
    query: QueryState<&Position>,
}

impl MySystem {
    fn new(world: &mut World) -> Self {
        Self {
            query: QueryState::<&Position>::new(world), // 只创建一次
        }
    }
    
    fn run(&mut self, world: &World) {
        for position in self.query.iter(world) { // 重用缓存的 QueryState
            // ...
        }
    }
}
```

### 2. 使用并行查询

```rust
// 对于大量实体的查询，使用 par_iter() 可以提高性能
fn process_large_dataset(query: Query<&Transform>) {
    query.par_iter().for_each(|transform| {
        // 在多个线程中并行处理
    });
}

// 使用 par_fold 进行聚合操作
fn calculate_average(query: Query<&Health>) -> f32 {
    let (sum, count) = query.par_fold(
        || (0, 0),
        |(sum, count), health| (sum + health.0, count + 1),
    );
    
    sum as f32 / count as f32
}
```

### 3. 注意查询的密集性

```rust
// 了解你的查询是密集的还是稀疏的
fn dense_query_example(query: Query<&Transform>) {
    // 这是一个密集查询
    // 数据在内存中是连续的，缓存命中率高
    // 适合访问大量实体
}

fn sparse_query_example(query: Query<&Transform, Added<Transform>>) {
    // 这是一个稀疏查询
    // 因为使用了 Added 过滤器
    // 适合访问少量实体
}
```

### 4. 避免在循环中创建查询

```rust
// 不好的做法：
fn process_entities(world: &World, entities: Vec<Entity>) {
    for entity in entities {
        let mut query = QueryState::<&Position>::new(world); // 每次循环都创建
        let position = query.get(world, entity);
        // ...
    }
}

// 好的做法：
fn process_entities(world: &World, entities: Vec<Entity>) {
    let mut query = QueryState::<&Position>::new(world); // 只创建一次
    for entity in entities {
        let position = query.get(world, entity); // 重用
        // ...
    }
}
```

### 5. 使用只读转换

```rust
// 将可变查询转换为只读查询可以提高性能
fn read_only_operation(query: Query<&mut Position>) {
    let readonly = query.as_readonly(); // 零成本转换
    
    // 可以同时使用多个只读迭代器
    let count1 = readonly.iter(world).count();
    let count2 = readonly.iter(world).count();
}
```

---

## 常见问题

### Q: QueryState 是线程安全的吗？

A: 是的，QueryState 是线程安全的。它只存储不可变的元数据，多个线程可以同时读取 QueryState。

### Q: QueryState 会自动更新吗？

A: 是的，QueryState 会在每次查询时自动检查原型是否有变化，如果有变化会自动更新匹配信息。

### Q: 我可以手动更新 QueryState 吗？

A: 是的，你可以使用 `update_archetypes()` 方法手动更新 QueryState 的匹配信息。

### Q: QueryState 的内存占用大吗？

A: QueryState 的内存占用取决于查询的复杂度和世界的大小。一般来说，QueryState 的内存占用是合理的，因为它只存储元数据而不是实际的组件数据。

### Q: 为什么需要同时维护 bitset 和向量？

A: 因为 bitset 适合快速判断某个表是否匹配（O(1) 时间复杂度），而向量适合快速遍历所有匹配的表（O(n) 时间复杂度，缓存友好）。同时维护两者可以兼顾查找和遍历的性能。

### Q: 什么是密集查询和稀疏查询？

A: 密集查询只访问表存储，数据在内存中是连续的，缓存命中率高，适合访问大量实体。稀疏查询需要访问原型存储，数据在内存中可能不连续，适合访问少量实体或有复杂过滤器的查询。

### Q: 如何判断查询是密集的还是稀疏的？

A: 查询的密集性由 `D::IS_DENSE && F::IS_DENSE` 决定。如果 QueryData 和 QueryFilter 都是密集的，那么查询就是密集的。

### Q: 0.19 版本有什么新特性？

A: 0.19 版本新增了并行折叠（par_fold）、多实体查询（get_many）、跟踪支持（par_iter_span）等特性，同时改进了性能和安全性。

---

## 附录

### 核心方法列表

| 方法 | 作用 | 版本 |
|------|------|------|
| `new(world)` | 创建一个新的 QueryState | 0.14+ |
| `try_new(world)` | 尝试创建一个新的 QueryState（可能失败） | 0.14+ |
| `query(world)` | 创建一个只读 Query | 0.14+ |
| `query_mut(world)` | 创建一个可变 Query | 0.14+ |
| `iter(world)` | 创建一个只读迭代器 | 0.14+ |
| `iter_mut(world)` | 创建一个可变迭代器 | 0.14+ |
| `par_iter(world)` | 创建一个并行只读迭代器 | 0.14+ |
| `par_iter_mut(world)` | 创建一个并行可变迭代器 | 0.14+ |
| `par_fold(world, init, func)` | 并行折叠操作 | 0.19+ |
| `get(world, entity)` | 获取特定实体的查询结果 | 0.14+ |
| `get_mut(world, entity)` | 获取特定实体的可变查询结果 | 0.14+ |
| `get_many(world, entities)` | 获取多个实体的查询结果 | 0.19+ |
| `get_many_mut(world, entities)` | 获取多个实体的可变查询结果 | 0.19+ |
| `single(world)` | 获取单个实体的查询结果 | 0.14+ |
| `single_mut(world)` | 获取单个实体的可变查询结果 | 0.14+ |
| `update_archetypes(world)` | 更新原型匹配信息 | 0.14+ |
| `as_readonly()` | 将 QueryState 转换为只读版本 | 0.14+ |

### 相关类型

- `QueryData`：查询数据的 trait
- `QueryFilter`：查询过滤器的 trait
- `Fetch`：从存储中获取数据的 trait
- `WorldQuery`：世界查询的 trait
- `Table`：表存储
- `Archetype`：原型存储
- `FilteredAccess`：组件访问信息
- `StorageId`：存储 ID（联合类型）

### 性能基准

| 操作 | 时间复杂度 | 说明 |
|------|------------|------|
| 创建 QueryState | O(A + T) | A 是原型数量，T 是表数量 |
| 迭代查询结果 | O(N) | N 是匹配的实体数量 |
| 并行迭代 | O(N/P + P) | P 是处理器核心数量 |
| 检查原型变化 | O(1) | 只需要比较版本号 |
| 更新匹配信息 | O(A + T) | 只在原型变化时执行 |
| 获取单个实体 | O(1) | 直接索引 |
| 获取多个实体 | O(K) | K 是实体数量 |

---

## 总结

### QueryState 的核心作用

1. **缓存查询元数据**：避免在每次系统运行时都重新计算匹配信息
2. **存储访问信息**：用于并行执行时的冲突检测
3. **优化迭代性能**：通过双重存储（bitset + 向量）提高查询速度
4. **跟踪原型变化**：只在必要时更新匹配信息
5. **支持并行查询**：通过 par_iter 和 par_fold 实现高效的并行处理

### 关键要点

- `QueryState` 是**长期存在**的（在系统运行之间缓存）
- `Query` 是**临时存在**的（只在系统运行时创建）
- `QueryState` 存储**元数据**，`Query` 存储**实际数据访问器**
- `QueryState` 使查询操作**快速且高效**
- 0.19 版本新增了**并行折叠**和**多实体查询**等重要特性

### 学习建议

1. **从简单开始**：先学习基本的查询用法（iter, get, single）
2. **理解密集性**：了解密集查询和稀疏查询的区别
3. **掌握并行查询**：学习使用 par_iter 和 par_fold 提高性能
4. **关注版本变化**：了解 0.19 版本的新特性
5. **实践优化**：根据最佳实践优化你的查询代码

---

## 相关文档

- [Bevy ECS 官方文档](https://bevyengine.org/learn/book/getting-started/ecs/)
- [Query 文档](https://docs.rs/bevy_ecs/latest/bevy_ecs/struct.Query.html)
- [World 文档](https://docs.rs/bevy_ecs/latest/bevy_ecs/struct.World.html)
- [0.19 版本发布说明](https://github.com/bevyengine/bevy/releases/tag/v0.19.0)

---

*本文档基于 Bevy ECS 0.19.0-dev 版本编写*

*最后更新：2026-01-20*
