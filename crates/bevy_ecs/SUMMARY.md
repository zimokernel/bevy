# Bevy ECS 总结

## 1. 模块结构

`bevy_ecs` 是 Bevy 游戏引擎的核心实体组件系统（ECS）实现。主要模块包括：

- **entity**: 实体管理和分配
- **component**: 组件定义和存储
- **query**: 查询系统
- **schedule**: 调度系统
- **system**: 系统定义和执行
- **world**: 世界管理
- **archetype**: 原型系统
- **storage**: 存储系统
- **event**: 事件系统
- **lifecycle**: 生命周期钩子
- **relationship**: 关系系统
- **hierarchy**: 层次结构系统

## 2. Entity 系统

### 核心概念

Entity 是游戏对象的唯一标识符，不包含任何数据，只是一个指向组件集合的引用。

### Entity 结构

```rust
/// Unique identifier for an entity in a [`World`].
/// Note that this is just an id, not the entity itself.
pub struct Entity {
    // 内部包含索引和生成号
}
```

### Entity 生命周期

1. **Spawn**: 通过 `World::spawn` 或 `Commands::spawn` 创建实体
2. **Update**: 通过 `Query`、`World::entity_mut` 或 `Commands::entity` 修改实体
3. **Despawn**: 通过 `Commands::despawn` 或 `World::despawn` 删除实体

### Entity 分配

实体分配分为两个阶段：
1. **Allocate**: 生成新的 Entity ID
2. **Spawn**: 使实体在 World 中"存在"

### EntityGeneration

跟踪 EntityIndex 的不同版本或生成号。重要的是，它可以回绕，意味着每个生成号不一定是唯一的。

```rust
pub struct EntityGeneration(u32);
```

### Entity 别名问题

当实体被 despawn 后，其 ID 可能被重新用于新实体，这称为别名。为防止此类错误，建议在知道实体已被 despawn 后立即停止持有 Entity 或 EntityGeneration 值。

## 3. Component 系统

### 核心概念

Component 是用于存储实体数据的数据类型。Component 是可派生的 trait，意味着可以通过 `#[derive(Component)]` 属性实现。

### Component 定义

```rust
/// A data type that can be used to store data for an [entity].
pub trait Component: Send + Sync + 'static {
    /// A constant indicating the storage type used for this component.
    const STORAGE_TYPE: StorageType;
    
    /// A marker type to assist Bevy with determining if this component is
    /// mutable, or immutable.
    type Mutability: ComponentMutability;
    
    // ... 其他方法
}
```

### Component 类型

- **Structs**: 带命名字段的结构体
- **Enums**: 枚举类型
- **Zero-sized types**: 零大小标记组件
- **Tuple structs**: 元组结构体

### 存储类型

1. **Table** (默认): 优化查询迭代
2. **SparseSet**: 优化组件插入和删除

```rust
#[derive(Component)]
#[component(storage = "SparseSet")]
struct ComponentA;
```

### Required Components

组件可以指定必需的组件。如果组件 A 需要组件 B，则当插入 A 时，B 也会被初始化和插入（如果未手动指定）。

```rust
#[derive(Component)]
#[require(B)]
struct A;

#[derive(Component, Default)]
struct B(usize);
```

### Component Hooks

可以为组件配置钩子函数：
- `on_add`: 组件添加时
- `on_insert`: 组件插入时
- `on_replace`: 组件替换时
- `on_remove`: 组件移除时

```rust
#[derive(Component, Debug)]
#[component(on_add)]
struct DoubleOnSpawn(usize);

impl DoubleOnSpawn {
    fn on_add(mut world: DeferredWorld, context: HookContext) {
        let mut entity = world.get_mut::<Self>(context.entity).unwrap();
        entity.0 *= 2;
    }
}
```

### ComponentMutability

组件可以是可变或不可变的：
- **Mutable**: 可以有 `&mut T` 引用
- **Immutable**: 保证永远不会有独占引用

```rust
#[derive(Component)]
#[component(immutable)]
struct ImmutableFoo;
```

## 4. Query 系统

### 核心概念

Query 是用于访问实体组件数据的主要方式。它允许系统查询满足特定条件的实体集合。

### Query 结构

```rust
/// A system parameter that provides access to a [`QueryState`] for the given [`QueryData`] and [`QueryFilter`].
pub struct Query<'w, 's, D: QueryData, F: QueryFilter = ()> {
    // ...
}
```

### QueryData

定义查询要获取的组件数据。可以是：
- `&T`: 不可变组件访问
- `&mut T`: 可变组件访问
- `Entity`: 实体 ID
- `Ref<T>`: 带变更检测的不可变访问
- `Mut<T>`: 带变更检测的可变访问
- `Has<T>`: 检查实体是否有指定组件
- `Option<&T>`: 可选组件访问

### QueryFilter

定义查询的过滤条件。常用过滤器：
- `With<T>`: 有组件 T 的实体
- `Without<T>`: 没有组件 T 的实体
- `Added<T>`: 自上次运行以来添加了组件 T 的实体
- `Changed<T>`: 自上次运行以来组件 T 发生变化的实体
- `Spawned`: 自上次运行以来生成的实体
- `Or<(A, B)>`: A 或 B
- `And<(A, B)>`: A 和 B

### Query 示例

```rust
// 基本查询
fn my_system(query: Query<(&Transform, &Velocity)>) {
    for (transform, velocity) in &query {
        // ...
    }
}

// 带过滤器的查询
fn my_system(query: Query<&Transform, With<Player>>) {
    for transform in &query {
        // ...
    }
}

// 可变查询
fn my_system(mut query: Query<&mut Transform>) {
    for mut transform in &mut query {
        transform.translation.x += 1.0;
    }
}

// 带变更检测的查询
fn my_system(query: Query<&Transform, Changed<Transform>>) {
    for transform in &query {
        // 只处理变换发生变化的实体
    }
}
```

### Query 组合

可以派生 QueryData 以创建可重用的查询结构：

```rust
#[derive(QueryData)]
struct MyQuery {
    entity: Entity,
    component_a: &'static ComponentA,
    component_b: &'static ComponentB,
}

fn my_system(query: Query<MyQuery>) {
    for q in &query {
        q.component_a;
    }
}
```

### QueryState

用于高效运行查询的状态存储：

```rust
let mut query_state = world.query::<&mut A>();
let mut mutable_component_values = query_state.get_many_mut(&mut world, entities).unwrap();
```

## 5. Schedule 系统

### 核心概念

Schedule 是系统的集合，以及运行它们所需的元数据和执行器。

### Schedule 结构

```rust
pub struct Schedule {
    label: InternedScheduleLabel,
    graph: ScheduleGraph,
    executable: SystemSchedule,
    executor: Box<dyn SystemExecutor>,
    executor_initialized: bool,
    warnings: Vec<ScheduleBuildWarning>,
}
```

### Schedule 标签

每个 schedule 都有一个 `ScheduleLabel` 值，用于唯一标识 schedule：

```rust
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash, Default)]
struct Update;
```

### Schedule 示例

```rust
fn hello_world() { println!("Hello world!") }

let mut world = World::new();
let mut schedule = Schedule::default();
schedule.add_systems(hello_world);

schedule.run(&mut world);
```

### 系统排序

可以指定系统的运行顺序：

```rust
schedule.add_systems((
    system_two,
    system_one.before(system_two),
    system_three.after(system_two),
));
```

### SystemSet

系统可以分组到 SystemSet 中：

```rust
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct PhysicsSet;

schedule.add_systems((
    system_one,
    system_two,
).in_set(PhysicsSet));
```

### ScheduleGraph

Schedule 的元数据容器：

```rust
pub struct ScheduleGraph {
    systems: Systems,
    system_sets: SystemSets,
    hierarchy: Dag<NodeId>,
    dependency: Dag<NodeId>,
    set_systems: DagGroups<SystemSetKey, SystemKey>,
    // ...
}
```

### ScheduleBuildSettings

配置 schedule 构建的设置：

```rust
pub struct ScheduleBuildSettings {
    ambiguity_detection: LogLevel,
    hierarchy_detection: LogLevel,
    auto_insert_apply_deferred: bool,
    use_shortnames: bool,
    report_sets: bool,
}
```

### 执行器

Schedule 使用执行器来运行系统。执行器负责并行化和优化系统执行。

## 6. 核心架构关系

### World、Entity、Component 和 Query 的关系

```
World (世界)
├── 包含多个 Entity (实体)
│   └── 每个 Entity 有多个 Component (组件)
├── 包含多个 Archetype (原型)
│   └── 每个 Archetype 存储具有相同组件集的实体
└── 提供 Query (查询) 接口
    └── Query 用于访问满足条件的实体组件
```

### 数据流向

1. **系统执行**: Schedule 运行系统
2. **查询数据**: 系统使用 Query 从 World 中获取数据
3. **修改数据**: 系统修改组件数据或使用 Commands 延迟修改
4. **应用修改**: Commands 在 ApplyDeferred 时应用

## 7. 关键设计模式

### 1. 延迟命令 (Deferred Commands)

```rust
fn my_system(mut commands: Commands) {
    commands.spawn(MyComponent); // 延迟到 ApplyDeferred 时执行
}
```

### 2. 系统参数 (SystemParam)

```rust
fn my_system(
    query: Query<&Transform>,
    mut commands: Commands,
    time: Res<Time>,
    asset_server: Res<AssetServer>,
) {
    // ...
}
```

### 3. 事件系统 (Event System)

```rust
#[derive(Event)]
struct MyEvent;

fn my_system(mut events: EventWriter<MyEvent>) {
    events.send(MyEvent);
}

fn my_other_system(mut events: EventReader<MyEvent>) {
    for event in events.iter() {
        // ...
    }
}
```

### 4. 层次结构 (Hierarchy)

```rust
fn my_system(mut commands: Commands) {
    let parent = commands.spawn_empty().id();
    commands.entity(parent).push_children(&[
        commands.spawn_empty().id(),
        commands.spawn_empty().id(),
    ]);
}
```

## 8. 性能考虑

### 1. 缓存友好

- Archetype 系统确保具有相同组件的实体在内存中连续存储
- Query 迭代时缓存命中率高

### 2. 并行执行

- Bevy ECS 自动并行化不冲突的系统
- 使用 `ambiguous_with` 解决潜在的冲突

### 3. 变更检测

- 只处理发生变化的组件
- 使用 `Changed<T>` 过滤器优化性能

### 4. 内存效率

- SparseSet 用于不常用的组件
- Table 用于常用的组件
- 避免不必要的组件复制

## 9. 最佳实践

### 1. 组件设计

- 组件应该是数据容器，不包含逻辑
- 按访问模式组织组件
- 避免过大的组件

### 2. 系统设计

- 系统应该小而专注
- 避免在系统中进行复杂计算
- 使用适当的查询过滤器

### 3. 实体管理

- 不要长时间持有 Entity ID
- 使用 Tag 组件而不是空实体
- 合理使用层次结构

### 4. 性能优化

- 使用 `Changed<T>` 过滤器减少不必要的处理
- 批量操作比单个操作更高效
- 合理使用系统集合和排序

## 10. 总结

Bevy ECS 是一个现代、高性能的实体组件系统，具有以下特点：

- **灵活性**: 支持多种组件类型和存储策略
- **性能**: 缓存友好的内存布局和自动并行化
- **易用性**: 直观的 API 和强大的派生宏
- **可扩展性**: 支持自定义系统参数、查询和调度

理解 Bevy ECS 的核心概念对于高效使用 Bevy 游戏引擎至关重要。通过合理设计组件、系统和查询，可以创建高性能、可维护的游戏代码。
