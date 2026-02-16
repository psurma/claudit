const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { getCurrentWindow } = window.__TAURI__.window;

let refreshTimer = null;
let countdownTimer = null;
let countdown = 60;
let isDetached = false;

const SESSION_MAX_AGE = 18000; // 5 hours
const WEEKLY_MAX_AGE = 604800; // 7 days in seconds
const THEME_KEY = "claudit-theme";
const COSTS_COLLAPSED_KEY = "claudit-costs-collapsed";
const STAY_ON_TOP_KEY = "claudit-stay-on-top";

const sparklineData = {};
const sparklineOffsets = {};

function initTheme() {
  const stored = localStorage.getItem(THEME_KEY);
  if (stored) {
    setTheme(stored);
  } else {
    const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
    setTheme(prefersDark ? "dark" : "light");
  }

  window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", (e) => {
    if (!localStorage.getItem(THEME_KEY)) {
      setTheme(e.matches ? "dark" : "light");
    }
  });
}

function setTheme(theme) {
  document.documentElement.setAttribute("data-theme", theme);
  updateThemeIcon(theme);
}

function toggleTheme() {
  const current = document.documentElement.getAttribute("data-theme");
  const next = current === "dark" ? "light" : "dark";
  localStorage.setItem(THEME_KEY, next);
  setTheme(next);
}

function updateThemeIcon(theme) {
  const moonIcon = document.getElementById("icon-moon");
  const sunIcon = document.getElementById("icon-sun");
  if (!moonIcon || !sunIcon) return;

  if (theme === "dark") {
    moonIcon.style.display = "";
    sunIcon.style.display = "none";
  } else {
    moonIcon.style.display = "none";
    sunIcon.style.display = "";
  }
}

async function fetchAndRender(silent = false) {
  const btn = document.getElementById("refresh-btn");
  if (!silent) btn.classList.add("spinning");

  try {
    const data = await invoke("get_all_data");
    renderUsage(data);
    renderCosts(data);
    document.getElementById("timestamp").textContent = "Updated " + data.timestamp;
  } catch (e) {
    console.error("Failed to fetch data:", e);
  } finally {
    if (!silent) btn.classList.remove("spinning");
    resetCountdown();
  }
}

function getHistoryForLabel(history, label) {
  if (!history || !Array.isArray(history)) return [];
  return history
    .filter((s) => s.buckets && s.buckets[label] !== undefined)
    .map((s) => ({ timestamp: s.timestamp, value: s.buckets[label] }));
}

function renderSparkline(dataPoints, maxAgeSeconds, color) {
  if (!dataPoints || dataPoints.length < 2) return "";

  const now = Math.floor(Date.now() / 1000);
  const cutoff = now - maxAgeSeconds;
  const filtered = dataPoints.filter((p) => p.timestamp >= cutoff);

  if (filtered.length < 2) return "";

  const width = 318;
  const height = 36;
  const padTop = 2;
  const padBottom = 2;
  const chartHeight = height - padTop - padBottom;

  const minTime = filtered[0].timestamp;
  const maxTime = filtered[filtered.length - 1].timestamp;
  const timeRange = maxTime - minTime || 1;

  // Values are 0-1 (percentage as decimal), clamp to 0-1
  const points = filtered.map((p) => ({
    x: ((p.timestamp - minTime) / timeRange) * width,
    y: padTop + chartHeight - Math.min(1, Math.max(0, p.value)) * chartHeight,
  }));

  const linePoints = points.map((p) => `${p.x.toFixed(1)},${p.y.toFixed(1)}`).join(" ");
  const areaPoints =
    `0,${height} ` +
    `${points[0].x.toFixed(1)},${points[0].y.toFixed(1)} ` +
    linePoints +
    ` ${points[points.length - 1].x.toFixed(1)},${height}`;

  const gradId = "sg-" + Math.random().toString(36).slice(2, 8);
  const compactPoints = JSON.stringify(filtered.map((p) => ({ t: p.timestamp, v: p.value })));

  return `<div class="sparkline" data-points='${compactPoints.replace(/'/g, "&#39;")}'>
    <svg width="${width}" height="${height}" viewBox="0 0 ${width} ${height}" preserveAspectRatio="none">
      <defs>
        <linearGradient id="${gradId}" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stop-color="${color}" stop-opacity="0.3"/>
          <stop offset="100%" stop-color="${color}" stop-opacity="0.05"/>
        </linearGradient>
      </defs>
      <polygon points="${areaPoints}" fill="url(#${gradId})"/>
      <polyline points="${linePoints}" fill="none" stroke="${color}" stroke-width="1.5" stroke-linejoin="round" stroke-linecap="round"/>
    </svg>
  </div>`;
}

function getColorForPct(pct) {
  if (pct >= 90) return "var(--red)";
  if (pct >= 70) return "var(--amber)";
  return "var(--green)";
}

function getMaxAgeForLabel(label) {
  if (label.toLowerCase().includes("session") || label.toLowerCase().includes("5hr")) {
    return SESSION_MAX_AGE;
  }
  return WEEKLY_MAX_AGE;
}

function isSessionLimit(label) {
  const lower = label.toLowerCase();
  return lower.includes("session") || lower.includes("5hr");
}

function getWindowBounds(offset, resetAt) {
  const end = resetAt + offset * SESSION_MAX_AGE;
  const start = end - SESSION_MAX_AGE;
  return { start, end };
}

function formatWindowLabel(start, end) {
  const startDate = new Date(start * 1000);
  const endDate = new Date(end * 1000);

  const formatTime = (d) => {
    const h = d.getHours();
    const ampm = h >= 12 ? "pm" : "am";
    const h12 = h % 12 || 12;
    const min = d.getMinutes();
    return min > 0 ? `${h12}:${String(min).padStart(2, "0")}${ampm}` : `${h12}${ampm}`;
  };

  const monthNames = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
  const sameDay = startDate.toDateString() === endDate.toDateString();

  if (sameDay) {
    return `${formatTime(startDate)} - ${formatTime(endDate)}`;
  }
  return `${monthNames[startDate.getMonth()]} ${startDate.getDate()} ${formatTime(startDate)} - ${monthNames[endDate.getMonth()]} ${endDate.getDate()} ${formatTime(endDate)}`;
}

function computePrediction(filtered, windowEnd) {
  if (filtered.length < 2) return null;
  const last = filtered[filtered.length - 1];
  if (last.timestamp >= windowEnd) return null;

  const n = filtered.length;
  let sumX = 0, sumY = 0, sumXX = 0, sumXY = 0;
  for (let i = 0; i < n; i++) {
    const x = filtered[i].timestamp;
    const y = filtered[i].value;
    sumX += x;
    sumY += y;
    sumXX += x * x;
    sumXY += x * y;
  }
  const denom = n * sumXX - sumX * sumX;
  if (Math.abs(denom) < 1e-12) return null;

  const slope = (n * sumXY - sumX * sumY) / denom;
  if (Math.abs(slope) < 1e-12) return null;

  const intercept = (sumY - slope * sumX) / n;
  const predicted = Math.min(1, Math.max(0, slope * windowEnd + intercept));
  return { predictedValue: predicted };
}

function renderWindowSparkline(dataPoints, start, end, color) {
  const filtered = dataPoints.filter((p) => p.timestamp >= start && p.timestamp <= end);

  const width = 318;
  const height = 36;

  if (filtered.length < 2) {
    return `<div class="sparkline">
      <svg width="${width}" height="${height}" viewBox="0 0 ${width} ${height}" preserveAspectRatio="none">
        <text x="${width / 2}" y="${height / 2 + 4}" text-anchor="middle" fill="var(--text-dim)" font-size="11" font-family="-apple-system, sans-serif">No data</text>
      </svg>
    </div>`;
  }

  const padTop = 2;
  const padBottom = 2;
  const chartHeight = height - padTop - padBottom;
  const timeRange = end - start || 1;

  const points = filtered.map((p) => ({
    x: ((p.timestamp - start) / timeRange) * width,
    y: padTop + chartHeight - Math.min(1, Math.max(0, p.value)) * chartHeight,
  }));

  const linePoints = points.map((p) => `${p.x.toFixed(1)},${p.y.toFixed(1)}`).join(" ");
  const areaPoints =
    `${points[0].x.toFixed(1)},${height} ` +
    linePoints +
    ` ${points[points.length - 1].x.toFixed(1)},${height}`;

  const gradId = "sg-" + Math.random().toString(36).slice(2, 8);
  const compactPoints = JSON.stringify(filtered.map((p) => ({ t: p.timestamp, v: p.value })));

  return `<div class="sparkline" data-points='${compactPoints.replace(/'/g, "&#39;")}'>
    <svg width="${width}" height="${height}" viewBox="0 0 ${width} ${height}" preserveAspectRatio="none">
      <defs>
        <linearGradient id="${gradId}" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stop-color="${color}" stop-opacity="0.3"/>
          <stop offset="100%" stop-color="${color}" stop-opacity="0.05"/>
        </linearGradient>
      </defs>
      <polygon points="${areaPoints}" fill="url(#${gradId})"/>
      <polyline points="${linePoints}" fill="none" stroke="${color}" stroke-width="1.5" stroke-linejoin="round" stroke-linecap="round"/>
      ${(() => {
        const now = Math.floor(Date.now() / 1000);
        if (now < start || now > end) return "";
        const prediction = computePrediction(filtered, end);
        if (!prediction) return "";
        const lastPt = points[points.length - 1];
        const predY = padTop + chartHeight - prediction.predictedValue * chartHeight;
        return `<line x1="${lastPt.x.toFixed(1)}" y1="${lastPt.y.toFixed(1)}" x2="${width}" y2="${predY.toFixed(1)}" stroke="${color}" stroke-width="1.5" stroke-dasharray="4 3" opacity="0.5"/>`;
      })()}
    </svg>
  </div>`;
}

function renderNavigableSparkline(label, dataPoints, color, resetAt) {
  const offset = sparklineOffsets[label] || 0;
  const { start, end } = getWindowBounds(offset, resetAt);
  const sparkline = renderWindowSparkline(dataPoints, start, end, color);
  const timeLabel = formatWindowLabel(start, end);

  const hasOlderData = dataPoints.some((p) => p.timestamp < start);
  const atCurrent = offset >= 0;

  const prevDisabled = !hasOlderData ? "disabled" : "";
  const nextDisabled = atCurrent ? "disabled" : "";

  return `<div class="sparkline-nav" data-label="${escapeHtml(label)}">
    <div class="sparkline-nav-header">
      <button class="sparkline-prev" ${prevDisabled}>&lt;</button>
      <span class="sparkline-range">${timeLabel}</span>
      <button class="sparkline-next" ${nextDisabled}>&gt;</button>
    </div>
    ${sparkline}
  </div>`;
}

function navigateSparkline(label, direction) {
  if (!sparklineOffsets[label]) sparklineOffsets[label] = 0;
  sparklineOffsets[label] += direction;
  if (sparklineOffsets[label] > 0) sparklineOffsets[label] = 0;

  const stored = sparklineData[label];
  if (!stored) return;

  const newHtml = renderNavigableSparkline(label, stored.dataPoints, stored.color, stored.resetAt);
  const container = document.querySelector(`.sparkline-nav[data-label="${label}"]`);
  if (container) {
    container.outerHTML = newHtml;
  }
}

function renderUsage(data) {
  const loading = document.getElementById("usage-loading");
  const errorEl = document.getElementById("usage-error");
  const limitsEl = document.getElementById("usage-limits");

  loading.style.display = "none";

  if (data.usage_error) {
    errorEl.style.display = "block";
    errorEl.textContent = data.usage_error;
    if (data.usage_error.toLowerCase().includes("unauthorized") || data.usage_error.toLowerCase().includes("session")) {
      const loginBtn = document.createElement("button");
      loginBtn.className = "login-btn";
      loginBtn.textContent = "Open claude to login";
      loginBtn.addEventListener("click", () => {
        invoke("open_login").catch((e) => console.error("open_login failed:", e));
      });
      errorEl.appendChild(document.createElement("br"));
      errorEl.appendChild(loginBtn);
    }
    limitsEl.innerHTML = "";
    return;
  }

  errorEl.style.display = "none";

  if (!data.usage || !data.usage.limits || data.usage.limits.length === 0) {
    limitsEl.innerHTML = '<div class="loading">No usage limits found</div>';
    return;
  }

  const history = data.usage_history;

  limitsEl.innerHTML = data.usage.limits
    .map((limit) => {
      const pct = Math.min(100, Math.round(limit.usage_pct * 100));
      const colorClass = pct >= 90 ? "red" : pct >= 70 ? "amber" : "green";
      const resetText = limit.reset_at ? formatReset(limit.reset_at) : "";

      const historyPoints = getHistoryForLabel(history, limit.label);
      const color = getColorForPct(pct);

      let sparkline;
      if (isSessionLimit(limit.label)) {
        const resetAt = limit.reset_at ? Math.floor(new Date(limit.reset_at).getTime() / 1000) : Math.floor(Date.now() / 1000);
        sparklineData[limit.label] = { dataPoints: historyPoints, color, resetAt };
        sparkline = renderNavigableSparkline(limit.label, historyPoints, color, resetAt);
      } else {
        const maxAge = getMaxAgeForLabel(limit.label);
        sparkline = renderSparkline(historyPoints, maxAge, color);
      }

      return `
        <div class="limit-item">
          <div class="limit-header">
            <span class="limit-label">${escapeHtml(limit.label)}</span>
            <span class="limit-pct" style="color: var(--${colorClass})">${pct}%</span>
          </div>
          <div class="progress-track">
            <div class="progress-fill ${colorClass}" style="width: ${pct}%"></div>
          </div>
          ${resetText ? `<div class="limit-reset">Resets ${resetText}</div>` : ""}
          ${sparkline}
        </div>
      `;
    })
    .join("");

  // Render extra usage (overages) if present
  const extraEl = document.getElementById("extra-usage");
  if (data.usage.extra_usage) {
    const eu = data.usage.extra_usage;
    const pct = Math.min(100, Math.round(eu.utilization * 100));
    const colorClass = pct >= 90 ? "red" : pct >= 70 ? "amber" : "green";
    extraEl.style.display = "block";
    extraEl.innerHTML = `
      <div class="limit-item">
        <div class="limit-header">
          <span class="limit-label">Extra Usage</span>
          <span class="limit-pct" style="color: var(--${colorClass})">${pct}% (&pound;${eu.used_credits.toFixed(2)})</span>
        </div>
        <div class="progress-track">
          <div class="progress-fill ${colorClass}" style="width: ${pct}%"></div>
        </div>
      </div>
    `;
  } else {
    extraEl.style.display = "none";
  }
}

function renderCosts(data) {
  const loading = document.getElementById("costs-loading");
  const errorEl = document.getElementById("costs-error");
  const dataEl = document.getElementById("costs-data");

  loading.style.display = "none";

  if (data.costs_error) {
    errorEl.style.display = "block";
    errorEl.textContent = data.costs_error;
    dataEl.style.display = "none";
    return;
  }

  errorEl.style.display = "none";

  if (!data.costs) {
    dataEl.style.display = "none";
    return;
  }

  dataEl.style.display = "block";
  document.getElementById("cost-today").textContent = formatCost(data.costs.today);
  document.getElementById("cost-week").textContent = formatCost(data.costs.week);
  document.getElementById("cost-month").textContent = formatCost(data.costs.month);
}

function formatCost(value) {
  if (value === 0) return "$0.00";
  return "$" + value.toFixed(2);
}

function formatReset(isoString) {
  try {
    const reset = new Date(isoString);
    const now = new Date();
    const diffMs = reset - now;

    if (diffMs <= 0) return "soon";

    const days = Math.floor(diffMs / 86400000);
    const hours = Math.floor((diffMs % 86400000) / 3600000);
    const minutes = Math.floor((diffMs % 3600000) / 60000);

    const parts = [];
    if (days > 0) parts.push(`${days}d`);
    if (hours > 0) parts.push(`${hours}h`);
    parts.push(`${minutes}m`);

    const dayNames = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
    const dayName = dayNames[reset.getDay()];
    const date = reset.getDate();
    const suffix = getOrdinalSuffix(date);
    const monthNames = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    const month = monthNames[reset.getMonth()];
    const h = reset.getHours();
    const ampm = h >= 12 ? "pm" : "am";
    const h12 = h % 12 || 12;
    const min = reset.getMinutes();
    const timeStr = min > 0 ? `${h12}:${String(min).padStart(2, "0")}${ampm}` : `${h12}${ampm}`;

    return `in ${parts.join(" ")} (${dayName} ${date}${suffix} ${month} ${timeStr})`;
  } catch {
    return "";
  }
}

function getOrdinalSuffix(n) {
  const s = ["th", "st", "nd", "rd"];
  const v = n % 100;
  return s[(v - 20) % 10] || s[v] || s[0];
}

function escapeHtml(text) {
  const div = document.createElement("div");
  div.textContent = text;
  return div.innerHTML;
}

// --- Preferences ---

let prefsOpen = false;

function togglePrefs() {
  prefsOpen = !prefsOpen;
  const section = document.getElementById("prefs-section");
  const btn = document.getElementById("prefs-btn");
  if (prefsOpen) {
    section.style.display = "block";
    btn.classList.add("active");
    loadPrefs();
  } else {
    section.style.display = "none";
    btn.classList.remove("active");
  }
}

async function loadPrefs() {
  try {
    const enabled = await invoke("get_autostart_enabled");
    document.getElementById("autostart-toggle").checked = enabled;
  } catch (e) {
    console.error("Failed to load autostart pref:", e);
  }

  const stayOnTop = localStorage.getItem(STAY_ON_TOP_KEY) === "true";
  document.getElementById("stay-on-top-toggle").checked = stayOnTop;
}

async function handleAutostartChange(e) {
  const enabled = e.target.checked;
  try {
    await invoke("set_autostart_enabled", { enabled });
  } catch (err) {
    console.error("Failed to set autostart:", err);
    e.target.checked = !enabled;
  }
}

async function handleStayOnTopChange(e) {
  const enabled = e.target.checked;
  localStorage.setItem(STAY_ON_TOP_KEY, enabled ? "true" : "false");
  try {
    await invoke("set_stay_on_top_pref", { enabled });
  } catch (err) {
    console.error("Failed to set stay-on-top pref:", err);
  }
}

async function checkForUpdates() {
  const statusEl = document.getElementById("update-status");
  statusEl.textContent = "Checking...";
  try {
    const info = await invoke("check_for_updates");
    document.getElementById("version-label").textContent = "v" + info.current_version;
    if (info.update_available) {
      statusEl.innerHTML =
        'v' + escapeHtml(info.latest_version) + ' available - <a href="#" class="download-link">Download</a>';
      statusEl.querySelector(".download-link").addEventListener("click", (e) => {
        e.preventDefault();
        invoke("open_url", { url: info.release_url }).catch((err) => console.error("open_url failed:", err));
      });
    } else if (info.latest_version === "unknown") {
      statusEl.textContent = "No releases found";
    } else {
      statusEl.textContent = "Up to date";
    }
  } catch (e) {
    console.error("Update check failed:", e);
    statusEl.textContent = "Check failed";
  }
}

// --- Costs collapse ---

function initCostsCollapse() {
  const collapsed = localStorage.getItem(COSTS_COLLAPSED_KEY) === "true";
  setCostsCollapsed(collapsed);
}

function toggleCostsCollapsed() {
  const content = document.getElementById("costs-content");
  const isCollapsed = content.classList.contains("collapsed");
  setCostsCollapsed(!isCollapsed);
}

function setCostsCollapsed(collapsed) {
  const content = document.getElementById("costs-content");
  const chevron = document.querySelector("#costs-header .chevron");
  if (collapsed) {
    content.classList.add("collapsed");
    chevron.classList.add("collapsed");
  } else {
    content.classList.remove("collapsed");
    chevron.classList.remove("collapsed");
  }
  localStorage.setItem(COSTS_COLLAPSED_KEY, collapsed ? "true" : "false");
}

function resetCountdown() {
  countdown = 60;
  if (countdownTimer) clearInterval(countdownTimer);
  countdownTimer = setInterval(() => {
    countdown--;
    if (countdown <= 0) {
      countdown = 60;
    }
    document.getElementById("next-refresh").textContent =
      "Auto-refresh in " + countdown + "s";
  }, 1000);
}

function startAutoRefresh() {
  if (refreshTimer) clearInterval(refreshTimer);
  refreshTimer = setInterval(fetchAndRender, 60000);
  resetCountdown();
}

function setDetachedUI(detached) {
  isDetached = detached;
  const popoutIcon = document.getElementById("icon-popout");
  const dockinIcon = document.getElementById("icon-dockin");
  const detachBtn = document.getElementById("detach-btn");
  if (detached) {
    document.body.classList.add("detached");
    popoutIcon.style.display = "none";
    dockinIcon.style.display = "";
    detachBtn.title = "Dock panel";
  } else {
    document.body.classList.remove("detached");
    popoutIcon.style.display = "";
    dockinIcon.style.display = "none";
    detachBtn.title = "Pop out panel";
  }
}

// Dismiss on Escape key (only when docked)
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && !isDetached) {
    invoke("hide_panel");
  }
});

function formatTooltipTime(ts) {
  const d = new Date(ts * 1000);
  const now = new Date();
  const isToday = d.toDateString() === now.toDateString();

  const h = d.getHours();
  const ampm = h >= 12 ? "pm" : "am";
  const h12 = h % 12 || 12;
  const min = d.getMinutes();
  const timeStr = min > 0 ? `${h12}:${String(min).padStart(2, "0")}${ampm}` : `${h12}${ampm}`;

  if (isToday) return timeStr;

  const monthNames = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
  return `${monthNames[d.getMonth()]} ${d.getDate()}, ${timeStr}`;
}

document.addEventListener("DOMContentLoaded", () => {
  initTheme();
  initCostsCollapse();

  // Shared sparkline tooltip
  const tooltip = document.createElement("div");
  tooltip.className = "sparkline-tooltip";
  document.getElementById("app").appendChild(tooltip);

  // Sync stay-on-top pref to Rust on startup
  const stayOnTop = localStorage.getItem(STAY_ON_TOP_KEY) === "true";
  invoke("set_stay_on_top_pref", { enabled: stayOnTop }).catch(() => {});

  document.getElementById("theme-toggle").addEventListener("click", toggleTheme);

  document.getElementById("refresh-btn").addEventListener("click", () => {
    fetchAndRender();
    startAutoRefresh();
  });

  document.getElementById("prefs-btn").addEventListener("click", togglePrefs);
  document.getElementById("autostart-toggle").addEventListener("change", handleAutostartChange);
  document.getElementById("stay-on-top-toggle").addEventListener("change", handleStayOnTopChange);
  document.getElementById("check-updates-link").addEventListener("click", (e) => {
    e.preventDefault();
    checkForUpdates();
  });
  document.getElementById("costs-header").addEventListener("click", toggleCostsCollapsed);

  // Sparkline navigation: delegated click handlers
  document.getElementById("usage-limits").addEventListener("click", (e) => {
    const btn = e.target.closest(".sparkline-prev, .sparkline-next");
    if (!btn || btn.disabled) return;
    const nav = btn.closest(".sparkline-nav");
    if (!nav) return;
    const label = nav.getAttribute("data-label");
    const direction = btn.classList.contains("sparkline-prev") ? -1 : 1;
    navigateSparkline(label, direction);
  });

  // Sparkline tooltip on hover
  document.getElementById("usage-limits").addEventListener("mousemove", (e) => {
    const sparkline = e.target.closest(".sparkline");
    if (!sparkline) {
      tooltip.style.display = "none";
      return;
    }
    const raw = sparkline.getAttribute("data-points");
    if (!raw) return;

    const pts = JSON.parse(raw);
    if (pts.length < 2) return;

    const svg = sparkline.querySelector("svg");
    if (!svg) return;

    const rect = svg.getBoundingClientRect();
    const mouseX = e.clientX - rect.left;
    const ratio = Math.max(0, Math.min(1, mouseX / rect.width));

    const minT = pts[0].t;
    const maxT = pts[pts.length - 1].t;
    const targetT = minT + ratio * (maxT - minT);

    let closest = pts[0];
    let closestDist = Math.abs(targetT - closest.t);
    for (let i = 1; i < pts.length; i++) {
      const dist = Math.abs(targetT - pts[i].t);
      if (dist < closestDist) {
        closest = pts[i];
        closestDist = dist;
      }
    }

    const pct = Math.round(Math.min(1, Math.max(0, closest.v)) * 100);
    tooltip.textContent = `${formatTooltipTime(closest.t)} \u2014 ${pct}%`;
    tooltip.style.display = "block";

    const tipRect = tooltip.getBoundingClientRect();
    const panelRect = document.querySelector(".panel").getBoundingClientRect();
    let left = e.clientX - tipRect.width / 2;
    left = Math.max(panelRect.left + 4, Math.min(left, panelRect.right - tipRect.width - 4));
    const top = rect.top - tipRect.height - 6;

    tooltip.style.left = left + "px";
    tooltip.style.top = top + "px";
  });

  document.getElementById("usage-limits").addEventListener("mouseleave", () => {
    tooltip.style.display = "none";
  });

  // Trackpad swipe navigation for sparklines
  let swipeAccum = 0;
  document.getElementById("usage-limits").addEventListener("wheel", (e) => {
    const nav = e.target.closest(".sparkline-nav");
    if (!nav) return;
    if (Math.abs(e.deltaX) < Math.abs(e.deltaY)) return;
    e.preventDefault();
    swipeAccum += e.deltaX;
    if (Math.abs(swipeAccum) >= 50) {
      const label = nav.getAttribute("data-label");
      const direction = swipeAccum > 0 ? -1 : 1;
      navigateSparkline(label, direction);
      swipeAccum = 0;
    }
  }, { passive: false });

  document.querySelector(".panel").addEventListener("mousedown", (e) => {
    if (!isDetached) return;
    if (e.target.closest("button")) return;
    if (e.target.closest(".section-header")) return;
    if (e.target.closest("label")) return;
    if (e.target.closest("a")) return;
    e.preventDefault();
    getCurrentWindow().startDragging();
  });

  document.getElementById("detach-btn").addEventListener("click", () => {
    if (isDetached) {
      invoke("attach_panel");
    } else {
      invoke("detach_panel");
    }
  });

  listen("panel-detached", () => {
    setDetachedUI(true);
  });

  listen("panel-attached", () => {
    setDetachedUI(false);
  });

  listen("panel-shown", () => {
    fetchAndRender(true);
    startAutoRefresh();
  });

  fetchAndRender();
  startAutoRefresh();
});
