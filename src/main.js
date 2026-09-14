const { invoke } = window.__TAURI__.core;

const listEl = document.getElementById("list");
const toastEl = document.getElementById("toast");
const settingsEl = document.getElementById("settings");
const sdkInput = document.getElementById("sdk-input");
const settingsNote = document.getElementById("settings-note");
const menuEl = document.getElementById("menu");
const menuToggle = document.getElementById("menu-toggle");
const aboutEl = document.getElementById("about");
const cardMenu = document.getElementById("card-menu");

// AVDs the user just acted on, so the UI reacts before adb catches up.
const pending = new Map();
let toastTimer;

function toast(message) {
  toastEl.textContent = message;
  toastEl.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (toastEl.hidden = true), 5000);
}

function statusText(avd) {
  if (pending.get(avd.name) === "start") return "Starting…";
  if (pending.get(avd.name) === "stop") return "Stopping…";
  if (!avd.running) return "Stopped";
  return avd.booted ? `Running · ${avd.serial}` : "Booting…";
}

// Headline is the Android version; the device falls back to it if unknown.
function titleText(avd) {
  return avd.android ?? avd.display ?? avd.name;
}

// Device name, API level and state, in that order of usefulness.
function subtitleText(avd) {
  const parts = [];
  const device = avd.display ?? avd.name;
  if (titleText(avd) !== device) parts.push(device);
  if (avd.api) parts.push(`API ${avd.api}`);
  parts.push(statusText(avd));
  return parts.join(" · ");
}

function render(avds) {
  // Re-rendering would tear the open per-card menu out from under the cursor;
  // the next poll picks the data up once it closes.
  if (!cardMenu.hidden) return;
  listEl.replaceChildren();

  if (!avds.length) {
    listEl.innerHTML =
      '<div class="empty"><strong>No emulators found</strong>' +
      "Create an AVD in Android Studio's Device Manager, then hit Refresh.</div>";
    return;
  }

  for (const avd of avds) {
    const busy = pending.has(avd.name);
    const card = document.createElement("div");
    card.className = "card";
    if (avd.running) card.classList.add("running");
    if (busy || (avd.running && !avd.booted)) card.classList.add("booting");

    const meta = document.createElement("div");
    meta.className = "meta";
    const name = document.createElement("div");
    name.className = "name";
    name.textContent = titleText(avd);
    const status = document.createElement("div");
    status.className = "status";
    status.textContent = subtitleText(avd);
    meta.append(name, status);

    // The primary action leads the row: play to start, square to stop.
    const btn = document.createElement("button");
    btn.disabled = busy;
    if (avd.running) {
      btn.className = "play stop";
      btn.title = "Stop";
      btn.innerHTML =
        '<svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">' +
        '<rect x="1.5" y="1.5" width="7" height="7" rx="1.6" fill="currentColor"/></svg>';
      btn.onclick = () => act(avd.name, "stop_avd", { serial: avd.serial });
    } else {
      btn.className = "play start";
      btn.title = "Start";
      btn.innerHTML =
        '<svg width="11" height="11" viewBox="0 0 11 11" aria-hidden="true">' +
        '<path d="M3 1.9 L9 5.5 L3 9.1 Z" fill="currentColor" stroke="currentColor" ' +
        'stroke-width="1.4" stroke-linejoin="round"/></svg>';
      btn.onclick = () => act(avd.name, "start_avd", { name: avd.name });
    }

    const more = document.createElement("button");
    more.className = "icon sm";
    more.title = "More";
    more.innerHTML =
      '<svg width="13" height="13" viewBox="0 0 14 14" aria-hidden="true">' +
      '<circle cx="7" cy="2.4" r="1.25" fill="currentColor"/>' +
      '<circle cx="7" cy="7" r="1.25" fill="currentColor"/>' +
      '<circle cx="7" cy="11.6" r="1.25" fill="currentColor"/></svg>';
    more.onclick = (e) => {
      e.stopPropagation();
      openCardMenu(avd, more);
    };

    card.append(btn, meta, more);
    listEl.append(card);
  }
}

async function act(avdName, command, args) {
  pending.set(avdName, command === "start_avd" ? "start" : "stop");
  await refresh();
  try {
    await invoke(command, args);
  } catch (err) {
    pending.delete(avdName);
    toast(String(err));
  }
  // Clear the optimistic state once adb has had time to reflect the change.
  setTimeout(() => {
    pending.delete(avdName);
    refresh();
  }, command === "start_avd" ? 12000 : 4000);
  refresh();
}

async function refresh() {
  try {
    render(await invoke("list_avds"));
  } catch (err) {
    listEl.innerHTML = "";
    const box = document.createElement("div");
    box.className = "empty";
    box.innerHTML = "<strong>Android SDK not found</strong>";
    box.append(String(err));
    listEl.append(box);
    openSettings();
  }
}

async function openSettings() {
  settingsEl.hidden = false;
  if (!sdkInput.value) {
    const current = await invoke("sdk_path");
    sdkInput.value = current ?? "";
    settingsNote.textContent = current
      ? "Detected automatically. Change it only if you use a different SDK."
      : "No SDK detected — paste the folder that contains 'emulator'.";
  }
}

document.getElementById("sdk-save").onclick = async () => {
  settingsNote.className = "note";
  try {
    const saved = await invoke("set_sdk_path", { path: sdkInput.value });
    sdkInput.value = saved;
    settingsNote.textContent = "Saved.";
    refresh();
  } catch (err) {
    settingsNote.className = "note error";
    settingsNote.textContent = String(err);
  }
};

sdkInput.addEventListener("keydown", (e) => {
  if (e.key === "Enter") document.getElementById("sdk-save").click();
});

/* ---------- per-emulator menu ---------- */

function closeCardMenu() {
  cardMenu.hidden = true;
}

function menuButton(label, onClick, enabled = true) {
  const b = document.createElement("button");
  b.className = "menu-item";
  b.textContent = label;
  b.disabled = !enabled;
  b.onclick = () => {
    closeCardMenu();
    onClick();
  };
  return b;
}

function openCardMenu(avd, anchor) {
  cardMenu.replaceChildren(
    menuButton("Open folder", () => reveal(avd), Boolean(avd.path)),
    menuButton("Force kill", () => forceKill(avd), avd.running),
  );

  // Anchor under the button, nudged left so it never leaves the window.
  const r = anchor.getBoundingClientRect();
  cardMenu.hidden = false;
  const width = cardMenu.offsetWidth;
  cardMenu.style.top = `${r.bottom + 4}px`;
  cardMenu.style.left = `${Math.max(6, Math.min(r.right - width, window.innerWidth - width - 6))}px`;
}

async function reveal(avd) {
  try {
    await invoke("open_avd_folder", { path: avd.path });
  } catch (err) {
    toast(String(err));
  }
}

async function forceKill(avd) {
  pending.set(avd.name, "stop");
  await refresh();
  try {
    await invoke("force_kill_avd", { name: avd.name, serial: avd.serial ?? null });
  } catch (err) {
    toast(String(err));
  }
  setTimeout(() => {
    pending.delete(avd.name);
    refresh();
  }, 2000);
}

/* ---------- theme ---------- */

// "auto" leaves the attribute off so the prefers-color-scheme rules apply.
function applyTheme(choice) {
  if (choice === "auto") document.documentElement.removeAttribute("data-theme");
  else document.documentElement.setAttribute("data-theme", choice);
  for (const item of menuEl.querySelectorAll("[data-theme]")) {
    item.setAttribute("aria-checked", String(item.dataset.theme === choice));
  }
}

function currentTheme() {
  try {
    return localStorage.getItem("theme") ?? "auto";
  } catch {
    return "auto";
  }
}

for (const item of menuEl.querySelectorAll("[data-theme]")) {
  item.onclick = () => {
    const choice = item.dataset.theme;
    applyTheme(choice);
    try {
      localStorage.setItem("theme", choice);
    } catch {
      /* private mode — the choice just won't survive a restart */
    }
  };
}

applyTheme(currentTheme());

/* ---------- menu ---------- */

function closeMenu() {
  menuEl.hidden = true;
  menuToggle.setAttribute("aria-expanded", "false");
}

menuToggle.onclick = (e) => {
  e.stopPropagation();
  closeCardMenu();
  const opening = menuEl.hidden;
  menuEl.hidden = !opening;
  menuToggle.setAttribute("aria-expanded", String(opening));
};

// Any click inside the menu is a completed action; anywhere else dismisses it.
document.addEventListener("click", (e) => {
  if (!menuEl.hidden && (menuEl.contains(e.target) || !menuToggle.contains(e.target))) {
    closeMenu();
  }
  if (!cardMenu.hidden && !cardMenu.contains(e.target)) closeCardMenu();
});

document.addEventListener("keydown", (e) => {
  if (e.key !== "Escape") return;
  if (!aboutEl.hidden) aboutEl.hidden = true;
  else {
    closeMenu();
    closeCardMenu();
  }
});

document.getElementById("menu-sdk").onclick = () => {
  settingsEl.hidden ? openSettings() : (settingsEl.hidden = true);
};

/* ---------- about ---------- */

document.getElementById("menu-about").onclick = async () => {
  try {
    const info = await invoke("about");
    document.getElementById("about-version").textContent = `Version ${info.version}`;
    document.getElementById("about-creator").textContent = info.creator;
  } catch {
    /* leave the placeholders in place */
  }
  aboutEl.hidden = false;
};

document.getElementById("about-close").onclick = () => (aboutEl.hidden = true);
aboutEl.onclick = (e) => {
  if (e.target === aboutEl) aboutEl.hidden = true;
};

refresh();
setInterval(refresh, 5000);
