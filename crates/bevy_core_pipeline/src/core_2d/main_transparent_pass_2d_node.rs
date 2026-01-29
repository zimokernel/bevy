use crate::core_2d::Transparent2d;
use bevy_ecs::prelude::*;
use bevy_render::{
    camera::ExtractedCamera,
    diagnostic::RecordDiagnostics,
    render_phase::ViewSortedRenderPhases,
    render_resource::{RenderPassDescriptor, StoreOp},
    renderer::{RenderContext, ViewQuery},
    view::{ExtractedView, ViewDepthTexture, ViewTarget},
};
use tracing::error;
#[cfg(feature = "trace")]
use tracing::info_span;

/// 2D 主透明渲染通道
/// 
/// 该函数负责渲染 2D 场景中的透明物体
/// 它会执行以下步骤:
/// 1. 获取视图实体和相关组件
/// 2. 获取透明渲染阶段
/// 3. 如果阶段为空则直接返回
/// 4. 创建渲染通道（加载深度缓冲区）
/// 5. 设置视口
/// 6. 渲染透明物体
/// 
/// 注意：透明通道会加载深度缓冲区但不写入，这样不透明物体可以遮挡透明物体
pub fn main_transparent_pass_2d(
    world: &World,
    view: ViewQuery<(
        &ExtractedCamera,
        &ExtractedView,
        &ViewTarget,
        &ViewDepthTexture,
    )>,
    transparent_phases: Res<ViewSortedRenderPhases<Transparent2d>>,
    // 透明物体渲染阶段
    mut ctx: RenderContext,
    // 渲染上下文
) {
    let view_entity = view.entity();
    // 获取视图实体
    let (camera, extracted_view, target, depth) = view.into_inner();
    // 获取视图内部组件

    let Some(transparent_phase) = transparent_phases.get(&extracted_view.retained_view_entity)
    else {
        return;
    };
    // 获取透明渲染阶段

    #[cfg(feature = "trace")]
    let _span = info_span!("main_transparent_pass_2d").entered();

    let diagnostics = ctx.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    // 获取诊断记录器

    let color_attachments = [Some(target.get_color_attachment())];
    // NOTE: For the transparent pass we load the depth buffer. There should be no
    // need to write to it, but store is set to `true` as a workaround for issue #3776,
    // https://github.com/bevyengine/bevy/issues/3776
    // so that wgpu does not clear the depth buffer.
    // As the opaque and alpha mask passes run first, opaque meshes can occlude
    // transparent ones.
    // 注意：对于透明通道，我们加载深度缓冲区。不需要写入它，但 store 设置为 `true` 作为问题 #3776 的解决方法
    // 这样 wgpu 不会清除深度缓冲区。由于不透明和 alpha 遮罩通道先运行，不透明网格可以遮挡透明物体
    let depth_stencil_attachment = Some(depth.get_attachment(StoreOp::Store));

    {
        let mut render_pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("main_transparent_pass_2d"),
            color_attachments: &color_attachments,
            depth_stencil_attachment,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        // 创建跟踪渲染通道
        let pass_span = diagnostics.pass_span(&mut render_pass, "main_transparent_pass_2d");

        if let Some(viewport) = camera.viewport.as_ref() {
            render_pass.set_camera_viewport(viewport);
        }
        // 设置相机视口

        if !transparent_phase.items.is_empty() {
            #[cfg(feature = "trace")]
            let _transparent_span = info_span!("transparent_main_pass_2d").entered();
            if let Err(err) = transparent_phase.render(&mut render_pass, world, view_entity) {
                error!("Error encountered while rendering the transparent 2D phase {err:?}");
            }
        }
        // 渲染透明物体

        pass_span.end(&mut render_pass);
    }

    // WebGL2 quirk: if ending with a render pass with a custom viewport, the viewport isn't
    // reset for the next render pass so add an empty render pass without a custom viewport
    // WebGL2 特性：如果以带有自定义视口的渲染通道结束，视口不会为下一个渲染通道重置，因此添加一个没有自定义视口的空渲染通道
    #[cfg(all(feature = "webgl", target_arch = "wasm32", not(feature = "webgpu")))]
    if camera.viewport.is_some() {
        #[cfg(feature = "trace")]
        let _reset_viewport_pass_2d = info_span!("reset_viewport_pass_2d").entered();

        let _reset_pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("reset_viewport_pass_2d"),
            color_attachments: &[Some(target.get_color_attachment())],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        // 创建空渲染通道以重置视口
    }
}
