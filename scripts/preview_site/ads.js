/* No-op ad hooks a portal SDK can replace later.
 *
 * A classic script, loaded by play.html. The game records a break or a
 * rewarded request in its state (`ad.break_slot` / `ad.reward_slot`). The page
 * forwards each new id here. Nothing is shown and nothing is granted: `break`
 * resolves `{shown:false}` and `rewarded` resolves `{rewarded:false}`.
 *
 * Plug a real SDK in by replacing `KagraAds.ads.break` and `.rewarded` (or
 * `KagraAds.create()`). Do not load a vendor script from this file.
 */
(function (root, factory) {
  const api = factory();
  if (typeof module === "object" && module.exports) module.exports = api;
  else root.KagraAds = api;
})(typeof globalThis !== "undefined" ? globalThis : this, function () {
  "use strict";

  function create() {
    const log = [];
    const ads = {
      log,
      break(slot) {
        const rec = { kind: "break", slot: String(slot || ""), at: Date.now() };
        log.push(rec);
        return Promise.resolve({ shown: false, slot: rec.slot, reason: "stub" });
      },
      rewarded(slot) {
        const rec = { kind: "rewarded", slot: String(slot || ""), at: Date.now() };
        log.push(rec);
        return Promise.resolve({ rewarded: false, slot: rec.slot, reason: "stub" });
      },
    };
    return ads;
  }

  const ads = create();
  return {
    create,
    ads,
    break(slot) {
      return ads.break(slot);
    },
    rewarded(slot) {
      return ads.rewarded(slot);
    },
  };
});
