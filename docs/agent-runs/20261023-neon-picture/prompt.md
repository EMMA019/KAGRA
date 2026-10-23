Extra visual review feedback for the current visual-fix PR (same Pixel screenshot). Please include all of this:

1) Ship: it's a blown-out white blob with an odd black hole in the middle. Lower the emissive, verify the Kenney GLB normals, textures and materials load correctly, keep the silhouette readable, and give it cyan/pink cockpit and engine glow. Pull the camera back or scale the ship to 0.7-0.8x so the next ring is clearly visible.

2) Trail: only floor dashes are visible and the ribbon isn't readable as a surface (the Z-to-Y fold may be edge-on to the camera). Make it wider, with a vivid magenta-to-cyan gradient, emitted right behind the ship.

3) Environment: add distance fog or a horizon gradient instead of a flat navy, maybe a reflective neon grid floor, and denser, brighter stars. Color-code the gates: cyan for rings to pass through, pink for hazards, instead of all-white wireframe. Light the turrets and rocks properly.

4) HUD: make BOOST a smaller, semi-transparent glassmorphism button with a neon inner glow. Hide the 'Corridor'/'Crest' links during play (show them only on the title screen). Remove the white horizontal lines at the top and bottom, or turn them into subtle neon lines. Fix the blown-out top HUD bar.

5) Feel: during boost, add radial speed lines/blur and a slight FOV widening or camera pull-back. Make the gate-pass spark bursts bigger and punchier.

Do several screenshot-review iterations in mobile portrait (WebGPU and WebGL2) until it looks clean and premium. Then continue with the queued code-review PR.
