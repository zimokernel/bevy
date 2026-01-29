#![expect(missing_docs, reason = "Not all docs are written yet, see #3492.")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]
#![doc(
    html_logo_url = "https://bevy.org/assets/icon.png",
    html_favicon_url = "https://bevy.org/assets/icon.png"
)]

//! Provides 2D sprite rendering functionality.

extern crate alloc;

mod mesh2d;
mod render;
mod sprite_mesh;
#[cfg(feature = "bevy_text")]
mod text2d;
mod texture_slice;
mod tilemap_chunk;

/// The sprite prelude.
///
/// This includes the most common types in this crate, re-exported for your convenience.
pub mod prelude {
    #[doc(hidden)]
    pub use crate::{ColorMaterial, MeshMaterial2d, SpriteMaterial};
}

use bevy_shader::load_shader_library;
pub use mesh2d::*;
pub use render::*;
pub use sprite_mesh::*;
pub(crate) use texture_slice::*;
pub use tilemap_chunk::*;

use bevy_app::prelude::*;
use bevy_asset::{embedded_asset, AssetEventSystems};
use bevy_core_pipeline::core_2d::{AlphaMask2d, Opaque2d, Transparent2d};
use bevy_ecs::prelude::*;
use bevy_image::{prelude::*, TextureAtlasPlugin};
use bevy_mesh::Mesh2d;
use bevy_render::{
    batching::sort_binned_render_phase, render_phase::AddRenderCommand,
    render_resource::SpecializedRenderPipelines, sync_world::SyncToRenderWorld, ExtractSchedule,
    Render, RenderApp, RenderStartup, RenderSystems,
};
use bevy_sprite::Sprite;

#[cfg(feature = "bevy_text")]
pub use crate::text2d::extract_text2d_sprite;

/// Adds support for 2D sprite rendering.
#[derive(Default)]
pub struct SpriteRenderPlugin;

/// System set for sprite rendering.
#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub enum SpriteSystems {
    ExtractSprites,
    ComputeSlices,
}

impl Plugin for SpriteRenderPlugin {
    fn build(&self, app: &mut App) {
        // ========== 加载着色器资源 ==========
        // 加载精灵视图绑定着色器库
        load_shader_library!(app, "render/sprite_view_bindings.wgsl");
        // 加载精灵着色器（内嵌资源）
        embedded_asset!(app, "render/sprite.wgsl");

        // ========== 注册依赖插件 ==========
        // 如果 TextureAtlasPlugin 尚未注册，则注册它
        // 纹理图集是精灵渲染的基础依赖
        if !app.is_plugin_added::<TextureAtlasPlugin>() {
            app.add_plugins(TextureAtlasPlugin);
        }

        // ========== 注册相关渲染插件 ==========
        app.add_plugins((
            Mesh2dRenderPlugin,        // 2D 网格渲染插件
            ColorMaterialPlugin,        // 颜色材质插件
            SpriteMeshPlugin,           // 精灵网格插件
            TilemapChunkPlugin,        // 瓦片地图块插件
            TilemapChunkMaterialPlugin, // 瓦片地图块材质插件
        ))
        // ========== 添加精灵切片计算系统 ==========
        .add_systems(
            PostUpdate,
            (
                // 在资源事件系统之前计算切片
                compute_slices_on_asset_event.before(AssetEventSystems),
                // 在精灵更改时计算切片
                compute_slices_on_sprite_change,
            )
                .in_set(SpriteSystems::ComputeSlices),
        );

        // ========== 注册精灵组件的同步要求 ==========
        // 确保 Sprite 组件会被同步到渲染世界
        app.register_required_components::<Sprite, SyncToRenderWorld>();

        // ========== 配置渲染子应用 ==========
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                // ========== 初始化渲染资源 ==========
                .init_resource::<ImageBindGroups>()
                .init_resource::<SpecializedRenderPipelines<SpritePipeline>>()
                .init_resource::<SpriteMeta>()
                .init_resource::<ExtractedSprites>()
                .init_resource::<ExtractedSlices>()
                .init_resource::<SpriteAssetEvents>()
                .init_resource::<SpriteBatches>()
                // ========== 添加渲染命令 ==========
                .add_render_command::<Transparent2d, DrawSprite>()
                // ========== 添加启动系统 ==========
                .add_systems(RenderStartup, init_sprite_pipeline)
                // ========== 添加提取阶段系统 ==========
                .add_systems(
                    ExtractSchedule,
                    (
                        // 提取精灵数据
                        extract_sprites.in_set(SpriteSystems::ExtractSprites),
                        // 提取精灵资源事件
                        extract_sprite_events,
                        // 提取 2D 文本精灵（当启用 bevy_text 特性时）
                        #[cfg(feature = "bevy_text")]
                        extract_text2d_sprite.after(SpriteSystems::ExtractSprites),
                    ),
                )
                // ========== 添加渲染阶段系统 ==========
                .add_systems(
                    Render,
                    (
                        // 队列精灵（在队列阶段）
                        queue_sprites
                            .in_set(RenderSystems::Queue)
                            .ambiguous_with(queue_material2d_meshes::<ColorMaterial>),
                        // 准备精灵图像绑定组
                        prepare_sprite_image_bind_groups.in_set(RenderSystems::PrepareBindGroups),
                        // 准备精灵视图绑定组
                        prepare_sprite_view_bind_groups.in_set(RenderSystems::PrepareBindGroups),
                        // 排序不透明 2D 渲染阶段
                        sort_binned_render_phase::<Opaque2d>.in_set(RenderSystems::PhaseSort),
                        // 排序 Alpha Mask 2D 渲染阶段
                        sort_binned_render_phase::<AlphaMask2d>.in_set(RenderSystems::PhaseSort),
                    ),
                );
        };
    }
}
