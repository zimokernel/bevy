use bevy_app::{Plugin, Update};
use bevy_ecs::{
    entity::Entity,
    query::{Added, Changed, Or},
    schedule::IntoScheduleConfigs,
    system::{Commands, Local, Query, Res, ResMut},
};

use bevy_asset::{Assets, Handle};

use bevy_image::TextureAtlasLayout;
use bevy_math::{primitives::Rectangle, vec2};
use bevy_mesh::{Mesh, Mesh2d};

use bevy_platform::collections::HashMap;
use bevy_sprite::{prelude::SpriteMesh, Anchor};

mod sprite_material;
pub use sprite_material::*;

use crate::MeshMaterial2d;

pub struct SpriteMeshPlugin;

impl Plugin for SpriteMeshPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        // 添加精灵材质插件
        app.add_plugins(SpriteMaterialPlugin);

        // 添加精灵网格相关系统
        app.add_systems(Update, (add_mesh, add_material).chain());
    }
}

// Insert a Mesh2d quad each time the SpriteMesh component is added.
// The meshhandle is kept locally so they can be cloned.
//
// 每当 SpriteMesh 组件被添加时，插入一个 Mesh2d 四边形。
// meshhandle 被保存在本地，以便可以克隆。
fn add_mesh(
    sprites: Query<Entity, Added<SpriteMesh>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut quad: Local<Option<Handle<Mesh>>>,
    mut commands: Commands,
) {
    // 如果本地缓存的四边形网格不存在，则创建一个
    if quad.is_none() {
        *quad = Some(meshes.add(Rectangle::from_size(vec2(1.0, 1.0))));
    }
    // 为每个新添加的 SpriteMesh 实体插入 Mesh2d 组件
    for entity in sprites {
        if let Some(quad) = quad.clone() {
            commands.entity(entity).insert(Mesh2d(quad));
        }
    }
}

// Change the material when SpriteMesh is added / changed.
//
// NOTE: This also adds the SpriteAtlasLayout into the SpriteMaterial,
// but this should instead be read later, similar to the images, allowing
// for hot reload.
//
// 当 SpriteMesh 被添加或更改时，更改材质。
//
// 注意：这也会将 SpriteAtlasLayout 添加到 SpriteMaterial 中，
// 但应该稍后读取，类似于图像，以支持热重载。
fn add_material(
    sprites: Query<
        (Entity, &SpriteMesh, &Anchor),
        Or<(Changed<SpriteMesh>, Changed<Anchor>, Added<Mesh2d>)>,
    >,
    texture_atlas_layouts: Res<Assets<TextureAtlasLayout>>,
    mut cached_materials: Local<HashMap<(SpriteMesh, Anchor), Handle<SpriteMaterial>>>,
    mut materials: ResMut<Assets<SpriteMaterial>>,
    mut commands: Commands,
) {
    for (entity, sprite, anchor) in sprites {
        // 检查是否有缓存的材质
        if let Some(handle) = cached_materials.get(&(sprite.clone(), *anchor)) {
            // 如果有缓存，则直接使用缓存的材质
            commands
                .entity(entity)
                .insert(MeshMaterial2d(handle.clone()));
        } else {
            // 如果没有缓存，则创建新的材质
            let mut material = SpriteMaterial::from_sprite_mesh(sprite.clone());
            material.anchor = **anchor;

            // 如果精灵使用了纹理图集，则将图集布局添加到材质中
            if let Some(texture_atlas) = &sprite.texture_atlas
                && let Some(texture_atlas_layout) =
                    texture_atlas_layouts.get(texture_atlas.layout.id())
            {
                material.texture_atlas_layout = Some(texture_atlas_layout.clone());
                material.texture_atlas_index = texture_atlas.index;
            }

            // 将材质添加到材质资源中
            let handle = materials.add(material);
            // 缓存材质，以便后续使用
            cached_materials.insert((sprite.clone(), *anchor), handle.clone());

            // 将材质组件插入到实体中
            commands
                .entity(entity)
                .insert(MeshMaterial2d(handle.clone()));
        }
    }
}
