//! Shows how to render a polygonal [`Mesh`], generated from a [`Rectangle`] primitive, in a 2D scene.
//! 
//! 演示如何在 2D 场景中渲染从 [`Rectangle`] 图元生成的多边形 [`Mesh`]。

use bevy::{color::palettes::basic::PURPLE, prelude::*};

fn main() {
    // 创建 Bevy 应用
    App::new()
        // 添加默认插件（包括渲染、输入、资产管理等）
        .add_plugins(DefaultPlugins)
        // 在 Startup 阶段添加 setup 系统
        .add_systems(Startup, setup)
        // 运行应用
        .run();
}

/// 初始化场景的系统
/// 
/// 此系统在应用启动时运行，用于创建相机和 2D 网格实体。
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    // 生成 2D 相机
    commands.spawn(Camera2d);

    // 生成 2D 网格实体
    commands.spawn((
        // Mesh2d 组件：将网格标记为 2D 网格
        // Rectangle::default() 创建一个默认的矩形网格
        Mesh2d(meshes.add(Rectangle::default())),
        // MeshMaterial2d 组件：为 2D 网格添加材质
        // Color::from(PURPLE) 使用紫色作为材质颜色
        MeshMaterial2d(materials.add(Color::from(PURPLE))),
        // Transform 组件：设置实体的变换（位置、旋转、缩放）
        // with_scale(Vec3::splat(128.)) 将网格缩放 128 倍
        Transform::default().with_scale(Vec3::splat(128.)),
    ));
}
