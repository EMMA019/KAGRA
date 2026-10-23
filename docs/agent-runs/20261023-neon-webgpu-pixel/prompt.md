Real Pixel 10 report after #147/#148 merged (stable play.html, almost certainly the WebGPU backend). Better, but several issues remain that emulation without WebGPU missed:

1. A black/white streaky, banded smear below and through the cyan ring, on the track. It looks like shadow acne, z-fighting between the trail ribbon and the floor, or a WebGPU-only depth/shadow bug. Find the root cause on the WebGPU path specifically (depth format/bias, the shadow compare sampler, ribbon depth write and blending).
2. Kenney GLB meteors and ships render flat white/unlit with no shading. Check that the GLB materials (Kenney models use vertex colors or a palette texture) get the base color and lighting on WebGPU, and tint them to fit the neon palette.
3. A thick solid cyan band across the upper screen at about 28% of the height, and a pink line at about 45%. Probably side rails or a horizon plane rendering as a big flat bar. Make it subtle.
4. The BOOST button still looks like the old pink circle and the Corridor/Crest links still show during play. Either the change isn't reaching the stable page (caching, or the page HTML wasn't updated) or it isn't applied. Verify the deployed play.html and fix it.

Since you have no WebGPU adapter, try harder to get one (Chrome with --enable-unsafe-webgpu --enable-features=Vulkan --use-angle=vulkan plus lavapipe/SwiftShader Vulkan, or wgpu native Vulkan lavapipe rendering the same path) and test the WebGPU backend directly. Open a PR, get CI green, never force-push, and report the root causes and WebGPU screenshots.
