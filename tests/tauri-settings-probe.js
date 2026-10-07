// Loaded only by the isolated, feature-gated Rust GUI regression binary.
// Uses production HTML/JS, unmocked Tauri IPC, and the actual native event loop.
window.addEventListener("DOMContentLoaded", async () => {
  const { invoke } = window.__TAURI__.core;
  const { emit } = window.__TAURI__.event;
  const pause = () => new Promise(resolve => setTimeout(resolve, 5));
  async function until(check) {
    const deadline = performance.now() + 4000;
    while (!await check()) {
      if (performance.now() > deadline) throw new Error("Settings did not finish rendering/deleting");
      await pause();
    }
  }
  try {
    await until(() => !document.getElementById("settings-view").hidden);
    const first = await invoke("get_snapshot");
    if (first.rules.length !== 0) throw new Error("Probe must start with empty isolated settings");
    for (let cycle = 0; cycle < 12; cycle++) {
      const rules = [0, 1].map(offset => ({
        id: cycle * 2 + offset + 1, mode: "host", source_app_id: null,
        pattern: `gui-probe-${cycle}-${offset}.invalid`, browser_id: "/Applications/Safari.app", enabled: true,
      }));
      await invoke("replace_rules", { rules });
      await until(() => document.querySelectorAll('[aria-label="Delete rule"]').length === 2);
      // A burst of real layout commands increases the overlap with native paint
      // callbacks. The old mutex scope deadlocks this actual event-loop path.
      const layouts = Array.from({ length: 8 }, () => invoke("set_picker_layout", {
        presentationRevision: first.presentation_revision, settings: true,
        displayedUrl: null, contentHeight: 500, contentWidth: 620,
      }));
      document.querySelector('[aria-label="Delete rule"]').click();
      await Promise.all(layouts);
      await until(() => document.querySelectorAll('[aria-label="Delete rule"]').length === 1);
      const after = await invoke("get_snapshot");
      if (after.rules.length !== 1 || after.rules[0].id !== rules[1].id) throw new Error("Wrong rule deleted");
      document.querySelector('[aria-label="Delete rule"]').click();
      await until(() => document.querySelectorAll('[aria-label="Delete rule"]').length === 0);
      if ((await invoke("get_snapshot")).rules.length !== 0) throw new Error("Deletion did not reach Rust");
    }
    await emit("browsefork-gui-probe-finished", "passed");
  } catch (error) {
    await emit("browsefork-gui-probe-finished", String(error));
  }
});
