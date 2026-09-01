// ==============================================================================
// Shao (哨兵) Frontend Dashboard Controller
// ==============================================================================

let fanGauge, tempGauge, powerGauge;
let chartCpu, chartPower, chartNetwork;

// Continuous Rolling Window State (Supports all ranges: Live 60s, 15m, 1h, 6h, 24h, 7d)
let activeTimeWindowSeconds = 60;
let activeDataPoints = [];
let isFetchingHistory = false;

// Dynamic Connection Watchdog
let eventSource = null;
let lastMessageTimestamp = 0;
let watchdogTimer = null;

document.addEventListener('DOMContentLoaded', async () => {
  initGauges();
  initDedicatedCharts();
  setupRangeButtons();
  await loadConfig();
  connectSSE();
  startWatchdog();
});

// -----------------------------------------------------------------------------
// Live Connection Watchdog & Status Pill
// -----------------------------------------------------------------------------
function setStatusOnline() {
  const pill = document.getElementById('status-pill');
  const dot = document.getElementById('status-dot');
  const text = document.getElementById('connection-status');
  if (pill && dot && text) {
    pill.className = 'flex items-center gap-2 px-3.5 py-1.5 rounded-full bg-emerald-500/10 border border-emerald-500/30 text-emerald-400 font-bold transition-all duration-300';
    dot.className = 'w-2.5 h-2.5 rounded-full bg-emerald-400 animate-pulse';
    text.textContent = 'LIVE 0.5s';
  }
}

function setStatusOffline(reason = 'OFFLINE') {
  const pill = document.getElementById('status-pill');
  const dot = document.getElementById('status-dot');
  const text = document.getElementById('connection-status');
  if (pill && dot && text) {
    pill.className = 'flex items-center gap-2 px-3.5 py-1.5 rounded-full bg-rose-500/10 border border-rose-500/30 text-rose-400 font-bold transition-all duration-300';
    dot.className = 'w-2.5 h-2.5 rounded-full bg-rose-500';
    text.textContent = reason;
  }
}

function setStatusConnecting() {
  const pill = document.getElementById('status-pill');
  const dot = document.getElementById('status-dot');
  const text = document.getElementById('connection-status');
  if (pill && dot && text) {
    pill.className = 'flex items-center gap-2 px-3.5 py-1.5 rounded-full bg-amber-500/10 border border-amber-500/30 text-amber-400 font-bold transition-all duration-300';
    dot.className = 'w-2.5 h-2.5 rounded-full bg-amber-400 animate-pulse';
    text.textContent = 'CONNECTING...';
  }
}

function startWatchdog() {
  if (watchdogTimer) clearInterval(watchdogTimer);
  watchdogTimer = setInterval(() => {
    const now = Date.now();
    if (now - lastMessageTimestamp > 1800) {
      setStatusOffline('OFFLINE');
      if (!eventSource || eventSource.readyState === EventSource.CLOSED) {
        connectSSE();
      }
    }
  }, 1000);
}

// -----------------------------------------------------------------------------
// 1. Radial Speedometer Gauges (24px Bold High-Contrast Values)
// -----------------------------------------------------------------------------
function initGauges() {
  const commonRadialOptions = {
    chart: { type: 'radialBar', height: 180, sparkline: { enabled: true } },
    plotOptions: {
      radialBar: {
        startAngle: -120,
        endAngle: 120,
        hollow: { size: '65%' },
        track: { background: 'rgba(255, 255, 255, 0.05)', strokeWidth: '100%' },
        dataLabels: {
          name: { show: false },
          value: {
            offsetY: 8,
            fontSize: '24px',
            fontWeight: 800,
            fontFamily: 'monospace',
            color: '#f8fafc',
            formatter: (val) => val,
          }
        }
      }
    },
    stroke: { lineCap: 'round' }
  };

  // Fan Gauge
  fanGauge = new ApexCharts(document.querySelector("#gauge-fan"), {
    ...commonRadialOptions,
    series: [46],
    colors: ['#22d3ee'],
    plotOptions: {
      ...commonRadialOptions.plotOptions,
      radialBar: {
        ...commonRadialOptions.plotOptions.radialBar,
        dataLabels: {
          ...commonRadialOptions.plotOptions.radialBar.dataLabels,
          value: {
            ...commonRadialOptions.plotOptions.radialBar.dataLabels.value,
            formatter: (val) => `${Math.round(val * 50)} RPM`
          }
        }
      }
    }
  });
  fanGauge.render();

  // Temp Gauge
  tempGauge = new ApexCharts(document.querySelector("#gauge-temp"), {
    ...commonRadialOptions,
    series: [41],
    colors: ['#10b981'],
    plotOptions: {
      ...commonRadialOptions.plotOptions,
      radialBar: {
        ...commonRadialOptions.plotOptions.radialBar,
        dataLabels: {
          ...commonRadialOptions.plotOptions.radialBar.dataLabels,
          value: {
            ...commonRadialOptions.plotOptions.radialBar.dataLabels.value,
            formatter: (val) => `${Math.round(val)}°C`
          }
        }
      }
    }
  });
  tempGauge.render();

  // Power Gauge
  powerGauge = new ApexCharts(document.querySelector("#gauge-power"), {
    ...commonRadialOptions,
    series: [13],
    colors: ['#f59e0b'],
    plotOptions: {
      ...commonRadialOptions.plotOptions,
      radialBar: {
        ...commonRadialOptions.plotOptions.radialBar,
        dataLabels: {
          ...commonRadialOptions.plotOptions.radialBar.dataLabels,
          value: {
            ...commonRadialOptions.plotOptions.radialBar.dataLabels.value,
            formatter: (val) => `${(val / 10.0).toFixed(2)} W`
          }
        }
      }
    }
  });
  powerGauge.render();
}

// -----------------------------------------------------------------------------
// 2. Dedicated Purpose-Built Time-Series Charts
// -----------------------------------------------------------------------------
function initDedicatedCharts() {
  const commonChartConfig = {
    chart: {
      type: 'area',
      height: 200,
      fontFamily: 'inherit',
      background: 'transparent',
      toolbar: { show: false },
      animations: { enabled: true, easing: 'linear', dynamicAnimation: { speed: 600 } }
    },
    theme: { mode: 'dark' },
    stroke: { curve: 'smooth', width: 2 },
    dataLabels: { enabled: false },
    grid: {
      borderColor: 'rgba(255, 255, 255, 0.05)',
      strokeDashArray: 4,
      yaxis: { lines: { show: true } }
    },
    xaxis: {
      type: 'datetime',
      labels: { style: { colors: '#64748b', fontSize: '10px' }, datetimeUTC: false },
      axisBorder: { show: false },
      axisTicks: { show: false }
    },
    fill: {
      type: 'gradient',
      gradient: { shadeIntensity: 1, opacityFrom: 0.35, opacityTo: 0.02, stops: [0, 95, 100] }
    },
    tooltip: { theme: 'dark', x: { format: 'HH:mm:ss' } }
  };

  // Graph 1: CPU Utilisation (%) vs CPU Clock (MHz)
  chartCpu = new ApexCharts(document.querySelector("#chart-cpu"), {
    ...commonChartConfig,
    colors: ['#6366f1', '#06b6d4'],
    yaxis: [
      {
        seriesName: 'CPU Load (%)',
        title: { text: 'CPU (%)', style: { color: '#818cf8', fontSize: '10px' } },
        min: 0,
        max: 100,
        labels: { style: { colors: '#64748b' }, formatter: (v) => `${Math.round(v)}%` }
      },
      {
        seriesName: 'Clock Speed (MHz)',
        opposite: true,
        title: { text: 'Clock (MHz)', style: { color: '#06b6d4', fontSize: '10px' } },
        min: 400,
        max: 3500,
        labels: { style: { colors: '#64748b' }, formatter: (v) => `${Math.round(v)} MHz` }
      }
    ],
    series: [
      { name: 'CPU Load (%)', data: [] },
      { name: 'Clock Speed (MHz)', data: [] }
    ]
  });
  chartCpu.render();

  // Graph 2: Power Draw (Watts)
  chartPower = new ApexCharts(document.querySelector("#chart-power"), {
    ...commonChartConfig,
    colors: ['#f59e0b'],
    yaxis: {
      title: { text: 'Power (W)', style: { color: '#f59e0b', fontSize: '10px' } },
      min: 0,
      labels: { style: { colors: '#64748b' }, formatter: (v) => `${v.toFixed(1)}W` }
    },
    series: [
      { name: 'Power Draw (W)', data: [] }
    ]
  });
  chartPower.render();

  // Graph 3: Network Throughput (Home LAN vs Tailscale VPN)
  chartNetwork = new ApexCharts(document.querySelector("#chart-network"), {
    ...commonChartConfig,
    height: 200,
    colors: ['#10b981', '#06b6d4', '#f59e0b', '#f43f5e'],
    yaxis: {
      title: { text: 'Throughput', style: { color: '#10b981', fontSize: '10px' } },
      min: 0,
      labels: {
        style: { colors: '#64748b' },
        formatter: (bps) => {
          if (bps < 1024) return `${bps.toFixed(0)} B/s`;
          if (bps < 1024 * 1024) return `${(bps / 1024).toFixed(1)} KB/s`;
          return `${(bps / (1024 * 1024)).toFixed(2)} MB/s`;
        }
      }
    },
    series: [
      { name: 'LAN Download (B/s)', data: [] },
      { name: 'LAN Upload (B/s)', data: [] },
      { name: 'VPN Download (B/s)', data: [] },
      { name: 'VPN Upload (B/s)', data: [] }
    ]
  });
  chartNetwork.render();
}

// -----------------------------------------------------------------------------
// 3. Connect to Server-Sent Events (SSE)
// -----------------------------------------------------------------------------
function connectSSE() {
  if (eventSource) {
    try { eventSource.close(); } catch(e) {}
  }

  setStatusConnecting();
  eventSource = new EventSource('/api/stream');

  eventSource.onopen = () => {
    lastMessageTimestamp = Date.now();
    setStatusOnline();
  };

  eventSource.onmessage = (event) => {
    try {
      lastMessageTimestamp = Date.now();
      setStatusOnline();
      const data = JSON.parse(event.data);
      updateDashboard(data);
    } catch (e) {
      console.error('Failed to parse SSE payload:', e);
    }
  };

  eventSource.onerror = () => {
    setStatusOffline('OFFLINE');
  };
}

// -----------------------------------------------------------------------------
// 4. Update UI with incoming Telemetry
// -----------------------------------------------------------------------------
function updateDashboard(data) {
  // Header & Dynamic Versioned Footer
  document.getElementById('server-host').textContent = `${data.system.hostname} • ${data.system.os_name} ${data.system.os_version}`;
  document.getElementById('uptime-display').textContent = data.system.uptime_human;
  const footerVer = document.getElementById('footer-version');
  if (footerVer && data.system.version) {
    footerVer.innerHTML = `Shao (哨兵) v${data.system.version} • <a href="https://github.com/adieltan/shao" target="_blank" class="text-brand-400 hover:underline">GitHub</a>`;
  }

  // CPU Load & Clock Frequency
  document.getElementById('cpu-percent').textContent = `${data.cpu.total_usage_percent.toFixed(1)}%`;
  document.getElementById('cpu-mhz-badge').textContent = `${data.cpu.avg_frequency_mhz} MHz`;
  document.getElementById('cpu-bar').style.width = `${Math.min(data.cpu.total_usage_percent, 100)}%`;

  // Memory
  document.getElementById('mem-percent').textContent = `${data.memory.usage_percent.toFixed(1)}%`;
  const usedGb = (data.memory.used_bytes / (1024 ** 3)).toFixed(1);
  const totalGb = (data.memory.total_bytes / (1024 ** 3)).toFixed(1);
  document.getElementById('mem-human').textContent = `${usedGb} / ${totalGb} GB`;
  document.getElementById('mem-bar').style.width = `${Math.min(data.memory.usage_percent, 100)}%`;

  // Disk Storage
  if (data.disk) {
    document.getElementById('disk-percent').textContent = `${data.disk.usage_percent.toFixed(1)}%`;
    document.getElementById('disk-human').textContent = `${data.disk.used_human} / ${data.disk.total_human}`;
    document.getElementById('disk-bar').style.width = `${Math.min(data.disk.usage_percent, 100)}%`;
    document.getElementById('disk-free').textContent = `Free: ${data.disk.available_human}`;
  }

  // Immich Stats
  if (data.immich) {
    document.getElementById('immich-photos').textContent = `${data.immich.photos.toLocaleString()} photos`;
    document.getElementById('immich-videos').textContent = `${data.immich.videos.toLocaleString()} vids`;
    document.getElementById('immich-storage').textContent = `${data.immich.usage_human}`;
    document.getElementById('immich-user').textContent = data.immich.user_name;
  }

  // Dockge Stats
  if (data.dockge) {
    document.getElementById('dockge-stacks').textContent = `${data.dockge.active_stacks} Stacks`;
    document.getElementById('dockge-containers').textContent = `${data.dockge.running_containers} Run`;
  }

  // Network Cards
  document.getElementById('lan-rx').textContent = data.network.lan_rx_speed_human;
  document.getElementById('lan-tx').textContent = data.network.lan_tx_speed_human;
  document.getElementById('lan-total').textContent = `${data.network.lan_combined_total_human} Total`;

  document.getElementById('vpn-rx').textContent = data.network.vpn_rx_speed_human;
  document.getElementById('vpn-tx').textContent = data.network.vpn_tx_speed_human;
  document.getElementById('vpn-total').textContent = `${data.network.vpn_combined_total_human} Total`;

  // Speedometer Gauges
  const fanPct = Math.min((data.thermals.fan_rpm / 5000.0) * 100.0, 100);
  fanGauge.updateSeries([Math.round(fanPct)]);

  const temp = data.thermals.cpu_temp_celsius;
  tempGauge.updateSeries([Math.round(temp)]);

  const watts = data.power.current_watts;
  powerGauge.updateSeries([Math.round(watts * 10)]);
  document.getElementById('cost-month').textContent = data.power.estimated_monthly_cost;
  document.getElementById('cost-year').textContent = data.power.estimated_annual_cost;
  document.getElementById('energy-today').textContent = data.power.energy_today_human;
  document.getElementById('energy-month').textContent = data.power.energy_month_human;
  document.getElementById('energy-year').textContent = data.power.energy_year_human;

  // --------------------------------------------------------------------------
  // Continuous Rolling Stream (Appends till NOW across ALL time windows)
  // --------------------------------------------------------------------------
  if (!isFetchingHistory) {
    const timestamp = data.timestamp * 1000;
    activeDataPoints.push({
      timestamp,
      cpu: data.cpu.total_usage_percent,
      cpu_freq: data.cpu.avg_frequency_mhz,
      power: data.power.current_watts,
      lan_rx: data.network.lan_rx_speed_bps,
      lan_tx: data.network.lan_tx_speed_bps,
      vpn_rx: data.network.vpn_rx_speed_bps,
      vpn_tx: data.network.vpn_tx_speed_bps,
    });

    // Prune points older than active time window
    const cutoff = timestamp - (activeTimeWindowSeconds * 1000);
    while (activeDataPoints.length > 0 && activeDataPoints[0].timestamp < cutoff) {
      activeDataPoints.shift();
    }

    renderAllCharts(activeDataPoints);
  }

  // Docker Containers
  updateDockerContainers(data.containers);
}

function renderAllCharts(points) {
  if (!points || points.length === 0) return;

  // Graph 1: CPU Utilisation (%) & CPU Clock (MHz)
  chartCpu.updateSeries([
    { name: 'CPU Load (%)', data: points.map(p => [p.timestamp, p.cpu]) },
    { name: 'Clock Speed (MHz)', data: points.map(p => [p.timestamp, p.cpu_freq]) }
  ]);

  // Graph 2: Power Draw
  chartPower.updateSeries([
    { name: 'Power Draw (W)', data: points.map(p => [p.timestamp, p.power]) }
  ]);

  // Graph 3: Network
  chartNetwork.updateSeries([
    { name: 'LAN Download (B/s)', data: points.map(p => [p.timestamp, p.lan_rx]) },
    { name: 'LAN Upload (B/s)', data: points.map(p => [p.timestamp, p.lan_tx]) },
    { name: 'VPN Download (B/s)', data: points.map(p => [p.timestamp, p.vpn_rx]) },
    { name: 'VPN Upload (B/s)', data: points.map(p => [p.timestamp, p.vpn_tx]) }
  ]);
}

// -----------------------------------------------------------------------------
// 5. Render Docker Containers
// -----------------------------------------------------------------------------
function updateDockerContainers(containers) {
  document.getElementById('container-count').textContent = containers.length;
  const grid = document.getElementById('docker-container-grid');
  grid.innerHTML = '';

  if (containers.length === 0) {
    grid.innerHTML = `
      <div class="col-span-2 text-center text-xs text-slate-500 py-6 font-mono">
        No active Docker containers detected on /var/run/docker.sock
      </div>
    `;
    return;
  }

  containers.forEach(c => {
    const isRunning = c.is_running;
    const card = document.createElement('div');
    card.className = 'p-3 rounded-xl bg-slate-900/70 border border-slate-800 flex items-center justify-between hover:border-slate-700 transition';
    card.innerHTML = `
      <div class="flex items-center gap-3 overflow-hidden">
        <span class="w-2.5 h-2.5 rounded-full ${isRunning ? 'bg-emerald-400 shadow-sm shadow-emerald-400/50' : 'bg-rose-500'}"></span>
        <div class="truncate">
          <h4 class="text-xs font-bold text-slate-200 truncate">${c.name}</h4>
          <p class="text-[10px] text-slate-400 truncate font-mono">${c.image}</p>
        </div>
      </div>
      <span class="text-[10px] px-2 py-0.5 rounded font-mono font-bold ${isRunning ? 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20' : 'bg-rose-500/10 text-rose-400 border border-rose-500/20'}">
        ${c.status}
      </span>
    `;
    grid.appendChild(card);
  });
}

// -----------------------------------------------------------------------------
// 6. Load Config & Apps (Icon + Name Only Shortcuts)
// -----------------------------------------------------------------------------
async function loadConfig() {
  try {
    const res = await fetch('/api/config');
    const cfg = await res.json();
    const container = document.getElementById('app-cards-container');
    container.innerHTML = '';

    (cfg.apps || []).forEach(app => {
      let targetUrl = app.url;
      try {
        const parsed = new URL(app.url, window.location.origin);
        if (parsed.hostname === '192.168.1.1' || parsed.hostname === '192.168.1.17' || parsed.hostname === 'localhost' || parsed.hostname === '127.0.0.1' || parsed.hostname === 'a456u') {
          targetUrl = `${window.location.protocol}//${window.location.hostname}:${parsed.port}${parsed.pathname}${parsed.search}`;
        }
      } catch (err) {
        if (app.url.startsWith(':')) {
          targetUrl = `${window.location.protocol}//${window.location.hostname}${app.url}`;
        }
      }

      const tile = document.createElement('a');
      tile.href = targetUrl;
      tile.target = '_blank';
      tile.className = 'p-3.5 rounded-xl bg-slate-900/70 border border-slate-800 flex items-center justify-between hover:border-brand-500/50 hover:bg-slate-800/80 transition group';
      tile.innerHTML = `
        <div class="flex items-center gap-3">
          <div class="p-2 rounded-lg bg-brand-500/10 text-brand-400 group-hover:bg-brand-500 group-hover:text-white transition">
            <i data-lucide="${app.icon || 'globe'}" class="w-4 h-4"></i>
          </div>
          <h4 class="text-sm font-bold text-white group-hover:text-brand-300 transition">${app.name}</h4>
        </div>
        <i data-lucide="external-link" class="w-4 h-4 text-slate-500 group-hover:text-brand-400 transition"></i>
      `;
      container.appendChild(tile);
    });

    lucide.createIcons();
  } catch (e) {
    console.error('Failed to load config:', e);
  }
}

// -----------------------------------------------------------------------------
// 7. Continuous Time-Range History Switching
// -----------------------------------------------------------------------------
function setupRangeButtons() {
  const buttons = document.querySelectorAll('.range-btn');
  buttons.forEach(btn => {
    btn.addEventListener('click', async () => {
      buttons.forEach(b => {
        b.classList.remove('bg-brand-500', 'text-white', 'font-extrabold', 'shadow');
        b.classList.add('text-slate-400');
      });
      btn.classList.add('bg-brand-500', 'text-white', 'font-extrabold', 'shadow');
      btn.classList.remove('text-slate-400');

      const seconds = parseInt(btn.dataset.seconds, 10);
      activeTimeWindowSeconds = seconds;

      if (seconds === 60) {
        const cutoff = Date.now() - 60000;
        activeDataPoints = activeDataPoints.filter(p => p.timestamp >= cutoff);
        renderAllCharts(activeDataPoints);
      } else {
        await fetchHistoricalWindow(seconds);
      }
    });
  });
}

async function fetchHistoricalWindow(seconds) {
  isFetchingHistory = true;
  try {
    const res = await fetch(`/api/history?seconds=${seconds}`);
    const points = await res.json();
    if (points && points.length > 0) {
      activeDataPoints = points.map(p => ({
        timestamp: p.timestamp * 1000,
        cpu: p.cpu_usage,
        cpu_freq: p.cpu_freq || 800,
        power: p.power_watts,
        lan_rx: p.lan_rx_speed,
        lan_tx: p.lan_tx_speed,
        vpn_rx: p.vpn_rx_speed,
        vpn_tx: p.vpn_tx_speed,
      }));
      renderAllCharts(activeDataPoints);
    }
  } catch (e) {
    console.error('Failed to fetch historical window:', e);
  } finally {
    isFetchingHistory = false;
  }
}
