Work on Emma's Windows PC. Do NOT clone. Repo D:\program\kagra.

origin/master is now b8b36f2 (RPG remaining closed). Emma asked to implement ROADMAP GAPS. Next: FPS close (item 5).

Already landed: look, fire (hitscan), hit/kill, first-person eye camera, muzzle/hit flash overlay.
Still missing vs roadmap: ammo, reload. Aim/hit exist.
Picture: 照準とマズルが読める — muzzle flash exists. Do NOT add SSAO. Shadow cascade only if the FPS dump is unreadable without it (probably skip).

Close THIS slice: dump-visible ammo count; fire spends a round; empty click does not kill; reload (R) restores ammo; verify world assertions. Keep eye camera and hitscan.