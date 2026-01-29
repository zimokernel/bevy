use bevy_material::labels::DrawFunctionId;

use crate::render_phase::{PhaseItem, TrackedRenderPass};
use bevy_app::{App, SubApp};
use bevy_ecs::{
    entity::Entity,
    query::{QueryEntityError, QueryState, ROQueryItem, ReadOnlyQueryData},
    resource::Resource,
    system::{ReadOnlySystemParam, SystemParam, SystemParamItem, SystemState},
    world::World,
};
use bevy_utils::TypeIdMap;
use core::{any::TypeId, fmt::Debug};
use std::sync::{PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};
use thiserror::Error;
use variadics_please::all_tuples;

/// A draw function used to draw [`PhaseItem`]s.
/// 用于绘制 [`PhaseItem`] 的 draw 函数
///
/// The draw function can retrieve and query the required ECS data from the render world.
/// Draw 函数可以从渲染世界中检索和查询所需的 ECS 数据
///
/// This trait can either be implemented directly or implicitly composed out of multiple modular
/// [`RenderCommand`]s. For more details and an example see the [`RenderCommand`] documentation.
/// 该特性可以直接实现,也可以通过多个模块化的 [`RenderCommand`] 隐式组成
/// 更多细节和示例请参见 [`RenderCommand`] 文档
pub trait Draw<P: PhaseItem>: Send + Sync + 'static {
    /// Prepares the draw function to be used. This is called once and only once before the phase
    /// begins. There may be zero or more [`draw`](Draw::draw) calls following a call to this function.
    /// Implementing this is optional.
    /// 准备 draw 函数以供使用.在阶段开始之前仅调用一次.
    /// 在此函数调用之后可能会有零次或多次 [`draw`](Draw::draw) 调用.
    /// 实现此方法是可选的.
    #[expect(
        unused_variables,
        reason = "The parameters here are intentionally unused by the default implementation; however, putting underscores here will result in the underscores being copied by rust-analyzer's tab completion."
    )]
    fn prepare(&mut self, world: &'_ World) {}

    /// Draws a [`PhaseItem`] by issuing zero or more `draw` calls via the [`TrackedRenderPass`].
    /// 通过 [`TrackedRenderPass`] 发出零次或多次 `draw` 调用来绘制 [`PhaseItem`]
    fn draw<'w>(
        &mut self,
        world: &'w World,
        pass: &mut TrackedRenderPass<'w>,
        view: Entity,
        item: &P,
    ) -> Result<(), DrawError>;
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum DrawError {
    #[error("Failed to execute render command {0:?}")]
    RenderCommandFailure(&'static str),
    #[error("Failed to get execute view query")]
    InvalidViewQuery,
    #[error("View entity not found")]
    ViewEntityNotFound,
}

/// Stores all [`Draw`] functions for the [`PhaseItem`] type.
/// 存储 [`PhaseItem`] 类型的所有 [`Draw`] 函数
///
/// For retrieval, the [`Draw`] functions are mapped to their respective [`TypeId`]s.
/// 为了检索, [`Draw`] 函数被映射到它们各自的 [`TypeId`]
pub struct DrawFunctionsInternal<P: PhaseItem> {
    pub draw_functions: Vec<Box<dyn Draw<P>>>,
    pub indices: TypeIdMap<DrawFunctionId>,
}

impl<P: PhaseItem> DrawFunctionsInternal<P> {
    /// Prepares all draw function. This is called once and only once before the phase begins.
    /// 准备所有 draw 函数.在阶段开始之前仅调用一次.
    pub fn prepare(&mut self, world: &World) {
        for function in &mut self.draw_functions {
            function.prepare(world);
        }
    }

    /// Adds the [`Draw`] function and maps it to its own type.
    /// 添加 [`Draw`] 函数并将其映射到其自身类型
    pub fn add<T: Draw<P>>(&mut self, draw_function: T) -> DrawFunctionId {
        self.add_with::<T, T>(draw_function)
    }

    /// Adds the [`Draw`] function and maps it to the type `T`
    /// 添加 [`Draw`] 函数并将其映射到类型 `T`
    pub fn add_with<T: 'static, D: Draw<P>>(&mut self, draw_function: D) -> DrawFunctionId {
        let id = DrawFunctionId(self.draw_functions.len().try_into().unwrap());
        self.draw_functions.push(Box::new(draw_function));
        self.indices.insert(TypeId::of::<T>(), id);
        id
    }

    /// Retrieves the [`Draw`] function corresponding to the `id` mutably.
    /// 可变地检索与 `id` 对应的 [`Draw`] 函数
    pub fn get_mut(&mut self, id: DrawFunctionId) -> Option<&mut dyn Draw<P>> {
        self.draw_functions.get_mut(id.0 as usize).map(|f| &mut **f)
    }

    /// Retrieves the id of the [`Draw`] function corresponding to their associated type `T`.
    /// 检索与关联类型 `T` 对应的 [`Draw`] 函数的 id
    pub fn get_id<T: 'static>(&self) -> Option<DrawFunctionId> {
        self.indices.get(&TypeId::of::<T>()).copied()
    }

    /// Retrieves the id of the [`Draw`] function corresponding to their associated type `T`.
    /// 检索与关联类型 `T` 对应的 [`Draw`] 函数的 id
    ///
    /// Fallible wrapper for [`Self::get_id()`]
    /// [`Self::get_id()`] 的可能失败的包装器
    ///
    /// ## Panics
    /// If the id doesn't exist, this function will panic.
    /// 如果 id 不存在,此函数将 panic
    pub fn id<T: 'static>(&self) -> DrawFunctionId {
        self.get_id::<T>().unwrap_or_else(|| {
            panic!(
                "Draw function {} not found for {}",
                core::any::type_name::<T>(),
                core::any::type_name::<P>()
            )
        })
    }
}

/// Stores all draw functions for the [`PhaseItem`] type hidden behind a reader-writer lock.
/// 存储 [`PhaseItem`] 类型的所有 draw 函数,隐藏在读写锁后面
///
/// To access them the [`DrawFunctions::read`] and [`DrawFunctions::write`] methods are used.
/// 要访问它们,请使用 [`DrawFunctions::read`] 和 [`DrawFunctions::write`] 方法
#[derive(Resource)]
pub struct DrawFunctions<P: PhaseItem> {
    internal: RwLock<DrawFunctionsInternal<P>>,
}

impl<P: PhaseItem> Default for DrawFunctions<P> {
    fn default() -> Self {
        Self {
            internal: RwLock::new(DrawFunctionsInternal {
                draw_functions: Vec::new(),
                indices: Default::default(),
            }),
        }
    }
}

impl<P: PhaseItem> DrawFunctions<P> {
    /// Accesses the draw functions in read mode.
    /// 以只读模式访问 draw 函数
    pub fn read(&self) -> RwLockReadGuard<'_, DrawFunctionsInternal<P>> {
        self.internal.read().unwrap_or_else(PoisonError::into_inner)
    }

    /// Accesses the draw functions in write mode.
    /// 以写入模式访问 draw 函数
    pub fn write(&self) -> RwLockWriteGuard<'_, DrawFunctionsInternal<P>> {
        self.internal
            .write()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// [`RenderCommand`]s are modular standardized pieces of render logic that can be composed into
/// [`Draw`] functions.
///
/// To turn a stateless render command into a usable draw function it has to be wrapped by a
/// [`RenderCommandState`].
/// This is done automatically when registering a render command as a [`Draw`] function via the
/// [`AddRenderCommand::add_render_command`] method.
///
/// Compared to the draw function the required ECS data is fetched automatically
/// (by the [`RenderCommandState`]) from the render world.
/// Therefore the three types [`Param`](RenderCommand::Param),
/// [`ViewQuery`](RenderCommand::ViewQuery) and
/// [`ItemQuery`](RenderCommand::ItemQuery) are used.
/// They specify which information is required to execute the render command.
///
/// Multiple render commands can be combined together by wrapping them in a tuple.
///
/// # Example
///
/// The `DrawMaterial` draw function is created from the following render command
/// tuple. Const generics are used to set specific bind group locations:
///
/// ```
/// # use bevy_render::render_phase::SetItemPipeline;
/// # struct SetMeshViewBindGroup<const N: usize>;
/// # struct SetMeshViewBindingArrayBindGroup<const N: usize>;
/// # struct SetMeshBindGroup<const N: usize>;
/// # struct SetMaterialBindGroup<M, const N: usize>(std::marker::PhantomData<M>);
/// # struct DrawMesh;
/// pub type DrawMaterial<M> = (
///     SetItemPipeline,
///     SetMeshViewBindGroup<0>,
///     SetMeshViewBindingArrayBindGroup<1>,
///     SetMeshBindGroup<2>,
///     SetMaterialBindGroup<M, 3>,
///     DrawMesh,
/// );
/// ```
pub trait RenderCommand<P: PhaseItem> {
    /// Specifies the general ECS data (e.g. resources) required by [`RenderCommand::render`].
    ///
    /// When fetching resources, note that, due to lifetime limitations of the `Deref` trait,
    /// [`SRes::into_inner`] must be called on each [`SRes`] reference in the
    /// [`RenderCommand::render`] method, instead of being automatically dereferenced as is the
    /// case in normal `systems`.
    ///
    /// All parameters have to be read only.
    ///
    /// [`SRes`]: bevy_ecs::system::lifetimeless::SRes
    /// [`SRes::into_inner`]: bevy_ecs::system::lifetimeless::SRes::into_inner
    type Param: SystemParam + 'static;
    /// Specifies the ECS data of the view entity required by [`RenderCommand::render`].
    ///
    /// The view entity refers to the camera, or shadow-casting light, etc. from which the phase
    /// item will be rendered from.
    /// All components have to be accessed read only.
    type ViewQuery: ReadOnlyQueryData;
    /// Specifies the ECS data of the item entity required by [`RenderCommand::render`].
    ///
    /// The item is the entity that will be rendered for the corresponding view.
    /// All components have to be accessed read only.
    ///
    /// For efficiency reasons, Bevy doesn't always extract entities to the
    /// render world; for instance, entities that simply consist of meshes are
    /// often not extracted. If the entity doesn't exist in the render world,
    /// the supplied query data will be `None`.
    type ItemQuery: ReadOnlyQueryData;

    /// Renders a [`PhaseItem`] by recording commands (e.g. setting pipelines, binding bind groups,
    /// issuing draw calls, etc.) via the [`TrackedRenderPass`].
    fn render<'w>(
        item: &P,
        view: ROQueryItem<'w, '_, Self::ViewQuery>,
        entity: Option<ROQueryItem<'w, '_, Self::ItemQuery>>,
        param: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult;
}

/// The result of a [`RenderCommand`].
#[derive(Debug)]
pub enum RenderCommandResult {
    Success,
    Skip,
    Failure(&'static str),
}

macro_rules! render_command_tuple_impl {
    ($(#[$meta:meta])* $(($name: ident, $view: ident, $entity: ident)),*) => {
        $(#[$meta])*
        impl<P: PhaseItem, $($name: RenderCommand<P>),*> RenderCommand<P> for ($($name,)*) {
            type Param = ($($name::Param,)*);
            type ViewQuery = ($($name::ViewQuery,)*);
            type ItemQuery = ($($name::ItemQuery,)*);

            #[expect(
                clippy::allow_attributes,
                reason = "We are in a macro; as such, `non_snake_case` may not always lint."
            )]
            #[allow(
                non_snake_case,
                reason = "Parameter and variable names are provided by the macro invocation, not by us."
            )]
            fn render<'w>(
                _item: &P,
                ($($view,)*): ROQueryItem<'w, '_, Self::ViewQuery>,
                maybe_entities: Option<ROQueryItem<'w, '_, Self::ItemQuery>>,
                ($($name,)*): SystemParamItem<'w, '_, Self::Param>,
                _pass: &mut TrackedRenderPass<'w>,
            ) -> RenderCommandResult {
                match maybe_entities {
                    None => {
                        $(
                            match $name::render(_item, $view, None, $name, _pass) {
                                RenderCommandResult::Skip => return RenderCommandResult::Skip,
                                RenderCommandResult::Failure(reason) => return RenderCommandResult::Failure(reason),
                                _ => {},
                            }
                        )*
                    }
                    Some(($($entity,)*)) => {
                        $(
                            match $name::render(_item, $view, Some($entity), $name, _pass) {
                                RenderCommandResult::Skip => return RenderCommandResult::Skip,
                                RenderCommandResult::Failure(reason) => return RenderCommandResult::Failure(reason),
                                _ => {},
                            }
                        )*
                    }
                }
                RenderCommandResult::Success
            }
        }
    };
}

all_tuples!(
    #[doc(fake_variadic)]
    render_command_tuple_impl,
    0,
    15,
    C,
    V,
    E
);

/// Wraps a [`RenderCommand`] into a state so that it can be used as a [`Draw`] function.
///
/// The [`RenderCommand::Param`], [`RenderCommand::ViewQuery`] and
/// [`RenderCommand::ItemQuery`] are fetched from the ECS and passed to the command.
pub struct RenderCommandState<P: PhaseItem + 'static, C: RenderCommand<P>> {
    state: SystemState<C::Param>,
    view: QueryState<C::ViewQuery>,
    entity: QueryState<C::ItemQuery>,
}

impl<P: PhaseItem, C: RenderCommand<P>> RenderCommandState<P, C> {
    /// Creates a new [`RenderCommandState`] for the [`RenderCommand`].
    pub fn new(world: &mut World) -> Self {
        Self {
            state: SystemState::new(world),
            view: world.query(),
            entity: world.query(),
        }
    }
}

impl<P: PhaseItem, C: RenderCommand<P> + Send + Sync + 'static> Draw<P> for RenderCommandState<P, C>
where
    C::Param: ReadOnlySystemParam,
{
    /// Prepares the render command to be used. This is called once and only once before the phase
    /// begins. There may be zero or more [`draw`](RenderCommandState::draw) calls following a call to this function.
    fn prepare(&mut self, world: &'_ World) {
        self.view.update_archetypes(world);
        self.entity.update_archetypes(world);
    }

    /// Fetches the ECS parameters for the wrapped [`RenderCommand`] and then renders it.
    fn draw<'w>(
        &mut self,
        world: &'w World,
        pass: &mut TrackedRenderPass<'w>,
        view: Entity,
        item: &P,
    ) -> Result<(), DrawError> {
        let param = self.state.get(world);
        let view = match self.view.get_manual(world, view) {
            Ok(view) => view,
            Err(err) => match err {
                QueryEntityError::NotSpawned(_) => return Err(DrawError::ViewEntityNotFound),
                QueryEntityError::QueryDoesNotMatch(_, _)
                | QueryEntityError::AliasedMutability(_) => {
                    return Err(DrawError::InvalidViewQuery)
                }
            },
        };

        let entity = self.entity.get_manual(world, item.entity()).ok();
        match C::render(item, view, entity, param, pass) {
            RenderCommandResult::Success | RenderCommandResult::Skip => Ok(()),
            RenderCommandResult::Failure(reason) => Err(DrawError::RenderCommandFailure(reason)),
        }
    }
}

/// Registers a [`RenderCommand`] as a [`Draw`] function.
/// They are stored inside the [`DrawFunctions`] resource of the app.
pub trait AddRenderCommand {
    /// Adds the [`RenderCommand`] for the specified render phase to the app.
    fn add_render_command<P: PhaseItem, C: RenderCommand<P> + Send + Sync + 'static>(
        &mut self,
    ) -> &mut Self
    where
        C::Param: ReadOnlySystemParam;
}

impl AddRenderCommand for SubApp {
    fn add_render_command<P: PhaseItem, C: RenderCommand<P> + Send + Sync + 'static>(
        &mut self,
    ) -> &mut Self
    where
        C::Param: ReadOnlySystemParam,
    {
        let draw_function = RenderCommandState::<P, C>::new(self.world_mut());
        let draw_functions = self
            .world()
            .get_resource::<DrawFunctions<P>>()
            .unwrap_or_else(|| {
                panic!(
                    "DrawFunctions<{}> must be added to the world as a resource \
                     before adding render commands to it",
                    core::any::type_name::<P>(),
                );
            });
        draw_functions.write().add_with::<C, _>(draw_function);
        self
    }
}

impl AddRenderCommand for App {
    fn add_render_command<P: PhaseItem, C: RenderCommand<P> + Send + Sync + 'static>(
        &mut self,
    ) -> &mut Self
    where
        C::Param: ReadOnlySystemParam,
    {
        SubApp::add_render_command::<P, C>(self.main_mut());
        self
    }
}
