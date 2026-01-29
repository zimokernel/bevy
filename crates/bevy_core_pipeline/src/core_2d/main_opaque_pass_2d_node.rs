use crate::core_2d::Opaque2d;
use bevy_ecs::prelude::*;
use bevy_render::{
    camera::ExtractedCamera,
    diagnostic::RecordDiagnostics,
    render_phase::ViewBinnedRenderPhases,
    render_resource::{RenderPassDescriptor, StoreOp},
    renderer::{RenderContext, ViewQuery},
    view::{ExtractedView, ViewDepthTexture, ViewTarget},
};
use tracing::error;
#[cfg(feature = "trace")]
use tracing::info_span;

use super::AlphaMask2d;

/// 2D 主不透明渲染通道
/// 
/// 该函数负责渲染 2D 场景中的不透明物体和 Alpha 遮罩物体
/// 它会执行以下步骤:
/// 1. 获取视图实体和相关组件
/// 2. 获取不透明和 Alpha 遮罩渲染阶段
/// 3. 如果两个阶段都为空则直接返回
/// 4. 创建渲染通道
/// 5. 设置视口
/// 6. 渲染不透明物体
/// 7. 渲染 Alpha 遮罩物体
pub fn main_opaque_pass_2d(
    world: &World,
    view: ViewQuery<(
        &ExtractedCamera,
        &ExtractedView,
        &ViewTarget,
        &ViewDepthTexture,
    )>,
    opaque_phases: Res<ViewBinnedRenderPhases<Opaque2d>>,
    // 不透明物体渲染阶段
    alpha_mask_phases: Res<ViewBinnedRenderPhases<AlphaMask2d>>,
    // Alpha 遮罩物体渲染阶段
    mut ctx: RenderContext,
    // 渲染上下文
) {
    let view_entity = view.entity();
    // 获取视图实体
    let (camera, extracted_view, target, depth) = view.into_inner();
    // 获取视图内部组件

    let (Some(opaque_phase), Some(alpha_mask_phase)) = (
        opaque_phases.get(&extracted_view.retained_view_entity),
        alpha_mask_phases.get(&extracted_view.retained_view_entity),
    ) else {
        return;
    };
    // 获取不透明和 Alpha 遮罩渲染阶段

    if opaque_phase.is_empty() && alpha_mask_phase.is_empty() {
        return;
    }
    // 如果两个阶段都为空则直接返回

    #[cfg(feature = "trace")]
    let _span = info_span!("main_opaque_pass_2d").entered();

    let diagnostics = ctx.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    // 获取诊断记录器

    let color_attachments = [Some(target.get_color_attachment())];
    let depth_stencil_attachment = Some(depth.get_attachment(StoreOp::Store));
    // 设置颜色和深度模板附件

    let mut render_pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("main_opaque_pass_2d"),
        color_attachments: &color_attachments,
        depth_stencil_attachment,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    // 创建跟踪渲染通道
    let pass_span = diagnostics.pass_span(&mut render_pass, "main_opaque_pass_2d");

    if let Some(viewport) = camera.viewport.as_ref() {
        render_pass.set_camera_viewport(viewport);
    }
    // 设置相机视口

    if !opaque_phase.is_empty() {
        #[cfg(feature = "trace")]
        let _opaque_span = info_span!("opaque_main_pass_2d").entered();
        if let Err(err) = opaque_phase.render(&mut render_pass, world, view_entity) {
            error!("Error encountered while rendering the 2d opaque phase {err:?}");
        }
    }
    // 渲染不透明物体

    if !alpha_mask_phase.is_empty() {
        #[cfg(feature = "trace")]
        let _alpha_mask_span = info_span!("alpha_mask_main_pass_2d").entered();
        if let Err(err) = alpha_mask_phase.render(&mut render_pass, world, view_entity) {
            error!("Error encountered while rendering the 2d alpha mask phase {err:?}");
        }
    }
    // 渲染 Alpha 遮罩物体

    pass_span.end(&mut render_pass);
    // 结束渲染通道
}
