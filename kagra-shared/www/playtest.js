// 「直してここ」 for the browser preview. No token. The page calls
// kagraPlaytest.attach(session, canvas) after the canvas has a size.
(function () {
  function phaseOf(type) {
    if (type === "pointerdown") return 0;
    if (type === "pointermove") return 1;
    if (type === "pointerup") return 2;
    return 3;
  }

  function canvasPoint(canvas, event) {
    const rect = canvas.getBoundingClientRect();
    const sx = rect.width > 0 ? canvas.width / rect.width : 1;
    const sy = rect.height > 0 ? canvas.height / rect.height : 1;
    return [(event.clientX - rect.left) * sx, (event.clientY - rect.top) * sy];
  }

  function attach(session, canvas) {
    if (!session || !canvas || canvas.dataset.kagraPlaytest === "1") return;
    canvas.dataset.kagraPlaytest = "1";
    try {
      session.setPlaytestEnabled(true);
      session.setPlaytestExternalChrome(true);
      session.setPlaytestDevice(navigator.userAgent || "browser", navigator.language || "");
    } catch (err) {
      console.warn("playtest unavailable", err);
      return;
    }

    const root = document.createElement("div");
    root.hidden = true;
    root.lang = "ja";
    root.style.cssText = [
      "position:fixed",
      "left:50%",
      "top:50%",
      "transform:translate(-50%,-50%)",
      "width:min(440px,calc(100vw - 24px))",
      "z-index:40",
      "padding:14px",
      "border-radius:12px",
      "background:rgba(28,32,44,.94)",
      "color:#ece8dc",
      "font:14px/1.4 sans-serif",
      "box-shadow:0 16px 40px rgba(0,0,0,.35)",
    ].join(";");
    root.innerHTML = [
      '<div style="color:#e8b054;font-weight:700;margin-bottom:8px">直してここ</div>',
      '<textarea lang="ja" rows="3" placeholder="メモ（日本語）" style="width:100%;box-sizing:border-box;background:#0e1018;color:#ece8dc;border:0;border-radius:8px;padding:8px"></textarea>',
      '<p data-status style="min-height:2.6em;margin:8px 0;font-size:13px"></p>',
      '<div style="display:flex;gap:8px">',
      '<button type="button" data-act="send" style="flex:1">送る</button>',
      '<button type="button" data-act="copy" style="flex:1">コピー</button>',
      '<button type="button" data-act="close" style="flex:1">閉じる</button>',
      "</div>",
    ].join("");
    document.body.appendChild(root);
    const note = root.querySelector("textarea");
    const status = root.querySelector("[data-status]");
    let lastSeq = 0;
    let shotUrl = "";

    function pullNote() {
      try { session.setPlaytestNote(note.value || ""); } catch (err) { console.warn(err); }
    }

    function saveShot() {
      const copy = document.createElement("canvas");
      copy.width = canvas.width;
      copy.height = canvas.height;
      const ctx = copy.getContext("2d");
      if (ctx) ctx.drawImage(canvas, 0, 0);
      copy.toBlob(function (blob) {
        if (!blob) return;
        const url = URL.createObjectURL(blob);
        shotUrl = url;
        const a = document.createElement("a");
        a.href = url;
        a.download = "kagra-playtest-" + Date.now() + ".png";
        a.click();
        if (navigator.clipboard && window.ClipboardItem) {
          navigator.clipboard.write([new ClipboardItem({ "image/png": blob })]).catch(function () {});
        }
        status.textContent = "スクリーンショットを保存しました。issue に貼り付けてください。";
      }, "image/png");
    }

    async function deliver(packet) {
      const text = packet.block || packet.copy || "";
      try {
        if (navigator.clipboard && text) await navigator.clipboard.writeText(text);
      } catch (err) {
        console.warn(err);
      }
      if (packet.action === "send" && packet.url) {
        window.open(packet.url, "_blank", "noopener");
      }
      status.textContent = packet.instruction || status.textContent;
      if (shotUrl) status.textContent += " 画像はダウンロード済みです。";
    }

    const orig = session.render.bind(session);
    session.render = function () {
      let state = {};
      try { state = JSON.parse(session.playtestStateJson()); } catch (err) { state = {}; }
      pullNote();
      if (state.shot) saveShot();
      const open = !!state.open;
      root.hidden = !open;
      if (open && document.activeElement !== note) {
        // Focus once; do not push the widget text back over an IME composition.
        if (!note.dataset.focused) {
          note.focus();
          note.dataset.focused = "1";
        }
      }
      if (!open) note.dataset.focused = "";
      if (state.packet && state.packet_seq !== lastSeq) {
        lastSeq = state.packet_seq;
        try { deliver(JSON.parse(session.playtestPacketJson())); } catch (err) { console.warn(err); }
      }
      return orig();
    };

    root.querySelectorAll("button").forEach(function (button) {
      button.addEventListener("click", function () {
        pullNote();
        session.playtestAction(button.getAttribute("data-act") || "close");
      });
    });

    function forward(event) {
      const pt = canvasPoint(canvas, event);
      let consumed = false;
      try {
        consumed = !!session.playtestPointer(event.pointerId || 0, pt[0], pt[1], phaseOf(event.type), event.pressure || 1);
      } catch (err) {
        consumed = false;
      }
      if (consumed) event.preventDefault();
    }
    canvas.addEventListener("pointerdown", forward);
    canvas.addEventListener("pointermove", forward);
    canvas.addEventListener("pointerup", forward);
    canvas.addEventListener("pointercancel", forward);

    window.addEventListener("keydown", function (event) {
      if (event.key !== "F12" && event.code !== "F12") return;
      event.preventDefault();
      try { session.pushKey("F12", true); } catch (err) { console.warn(err); }
    });
  }

  window.kagraPlaytest = { attach: attach };
})();
