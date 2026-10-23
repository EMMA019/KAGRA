# Result

Portrait lavapipe frame (same shaders the WebGPU page runs; this
headless Chrome will not composite a WebGPU canvas, see session.md):

`/opt/cursor/artifacts/screenshots/lumen-lavapipe-portrait.png`

WebGPU attach on that Chrome: adapter present, `depth32float-filterable`
false, surface `Rgba8Unorm`, BOOST `border-radius: 18px` and class `on`,
Corridor/Crest `hidden` during play.

Stable `play.html` after Pages Deploy `39946193074` (11:27:53Z) already
contains the glass button and `showLinks`. This change keeps that and
stops a WebGPU canvas layer from covering it.
