# Session — normals + USB pad

- 頂点ストライドは 32 のまま。TBN は MToon と同じ cotangent frame（接空間頂点を足さない）。
- Mesh3D の group 1 を diffuse + sampler + `t_normal` に拡張。`mesh_mat.params.z` が法線フラグ。
- 法線テクスチャは線形（`load(..., srgb=False)`）。sRGB デコードするとバンプが歪む。
- glTF flatten は最初のマテリアルの `baseColorTexture` / `normalTexture`。無ければ images[0] をアルベド。
- パッド: Python 面は既にあった。gilrs 0.10.10 を EventLoop スレッドで pump。Windows はループ 1 本。
- `inject_pad` が実機より優先。CI / スモークは今までどおり inject。
- この VM に `kagra_core` GPU wheel は無い。画素は閉じないと書いた。
