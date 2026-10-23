# Post pass graph

Branch `cursor/post-pass-2c16`, from `origin/master` at the linear-color merge (`1a4b53b`). Separate from the glTF PR.

## What landed

- `PostGraph` in `kagra-shared/src/render/post.rs` plus `post.wgsl`. Each pass is one fullscreen entry, its own uniform buffer, and a color target. No storage buffers. Samples stay in uniform control flow.
- Order when a dump opts in: half-res SSAO → DOF → camera motion → existing bloom grade → TAA (or FXAA) → vignette. Omitted fields leave SSAO, DOF, motion, and TAA off, so the historical composite stays.
- `WorldDoc` fields: `ssao`, `taa`, `dof_focus`, `dof_aperture`, `motion_blur`. `KAGRA_QUALITY=0` / `off` / `low` forces them off. `high` / `2` enables SSAO radius 8 and TAA only when those fields are absent. DOF and motion are never forced.
- TAA uses a Halton jitter on the rendered view-projection only. Motion stores the unjittered matrix. The first TAA frame copies the current color. TAA replaces FXAA, and vignette runs after it with the same curve as `bloom.wgsl`.

## Depth

Packing device depth into the color alpha failed. The 3D pipeline is `ALPHA_BLENDING`, so the written depth blended toward the clear and the color picked up a tint. SSAO barely darkened (floor 220→216). DOF treated every pixel as the far plane.

The fix is a depth prepass (`fs_depth`) after the color pass. It loads the existing depth buffer, does not write depth, and outputs linear eye depth in meters into its own target (the HDR frame format). Sky is the far plane. Cutout holes discard after the albedo sample. SSAO compares that eye depth against the local depth plane so a flat floor does not occlude itself. DOF reads the eye depth directly. Motion converts it back to device z for the unproject. The color pipeline blend is unchanged.

Master later disabled the float frame on browser WebGPU (`#152`). The merge keeps that path: when the frame format is the swapchain, `direct` is true and this graph does not run, so the phone does not sample `Rgba16Float`. The shadow compare sampler stays nearest. `shadow.z` / `shadow.w` are still near and far for the HDR depth prepass only.

`#149` then landed glTF PBR on master. This merge keeps that double-sided color pipeline and the depth prepass as two pipelines. The double-sided shadow pipeline uses the same depth32float bias as the culled shadow pipeline (constant 8192, slope 1.5, clamp 0.008).

## Checks

Lavapipe (`VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json`):

- `offscreen_ssao_darkens_contact_and_keeps_the_clear` passed. Contact darkens; the clear corner stays.
- `offscreen_dof_blurs_the_far_box` failed once because the clear `(40, 80, 160)` matched the “far blue” mask. The far box itself (30, 40, 220) moves by mean channel delta ~35 versus ~6.5 on the in-focus red box. The mask now excludes the clear.
- `offscreen_motion_blur_is_quiet_until_the_camera_moves` and `offscreen_taa_keeps_a_flat_center` passed.
- `offscreen_clear_matches_reference_on_srgb_and_unorm` passed. New passes stay off, so the historical clear is unchanged.

Screenshots: `post-ssao-off.png`, `post-ssao-on.png`, `post-dof-sharp.png`, `post-dof-blur.png`, `post-motion-sharp.png`, `post-motion-blur.png`, `post-taa.png`.
