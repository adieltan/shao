// ==============================================================================
// Shao (哨兵) Frontend Dashboard Controller
// ==============================================================================

let fanGauge, tempGauge, powerGauge, mainChart;
let isLiveStreaming = true;
let rollingHistory = [];
const MAX_LIVE_POINTS = 60;

document.addEventListener('DOMContentLoaded', async () => {
  initGauges();
  initMainChart();
  setupRangeButtons();
  await loadConfig();
  connectSSE();
});

// 1. Radial Speedometer Gauges
function initGauges() {
  const commonRadialOptions = {
    chart: { type: 'radialBar', height: 180, sparkline: { enabled: true } },
    plotOptions: {
      radialBar: {
        startAngle: -120,
        endAngle: 120,
        hollow: { size: '68%' },
        track: { background: 'rgba(255, 255, 255, 0.05)', strokeWidth: '100%' },
        dataLabels: {
          name: { show: false },
          value: {
            offsetY: 8,
            fontSize: '18px',
            fontWeight: 700,
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
    series: [46], // default 2300 RPM on 5000 max scale
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
    series: [13], // 1.3W on 10W scale
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

// 2. Main Time-Series Streaming Chart
function initMainChart() {
  const options = {
    chart: {
      type: 'area',
      height: 280,
      fontFamily: 'inherit',
      background: 'transparent',
      toolbar: { show: false },
      animations: {
        enabled: true,
        easing: 'linear',
        dynamicAnimation: { speed: 1000 }
      }
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
      labels: {
        style: { colors: '#64748b', fontSize: '11px' },
        datetimeUTC: false
      },
      axisBorder: { show: false },
      axisTicks: { show: false }
    },
    yaxis: [
      {
        seriesName: 'CPU Load',
        title: { text: 'CPU (%)', style: { color: '#818cf8', fontSize: '11px' } },
        min: 0,
        max: 100,
        labels: { style: { colors: '#64748b' } }
      },
      {
        seriesName: 'Power Draw',
        opposite: true,
        title: { text: 'Power (Watts)', style: { color: '#f59e0b', fontSize: '11px' } },
        min: 0,
        labels: {
          style: { colors: '#64748b' },
          formatter: (v) => `${v.toFixed(1)}W`
        }
      }
    ],
    colors: ['#6366f1', '#f59e0b', '#22d3ee'],
    fill: {
      type: 'gradient',
      gradient: {
        shadeIntensity: 1,
        opacityFrom: 0.35,
        opacityTo: 0.02,
        stops: [0, 95, 100]
      }
    },
    series: [
      { name: 'CPU Load (%)', data: [] },
      { name: 'Power Draw (W)', data: [] },
      { name: 'CPU Temp (°C)', data: [] }
    ],
    tooltip: {
      theme: 'dark',
      x: { format: 'HH:mm:ss' }
    }
  };

  mainChart = new ApexCharts(document.querySelector("#chart-telemetry"), options);
  mainChart.render();
}

// 3. Connect to Server-Sent Events (SSE)
function connectSSE() {
  const statusElem = document.getElementById('connection-status');
  const es = new EventSource('/api/stream');

  es.onopen = () => {
    statusElem.textContent = 'LIVE 1.0s';
    statusElem.parentElement.classList.remove('bg-rose-500/10', 'border-rose-500/30', 'text-rose-400');
    statusElem.parentElement.classList.add('bg-emerald-500/10', 'border-emerald-500/30', 'text-emerald-400');
  };

  es.onmessage = (event) => {
    try {
      const data = JSON.parse(event.data);
      updateDashboard(data);
    } catch (e) {
      console.error('Failed to parse SSE payload:', e);
    }
  };

  es.onerror = () => {
    statusElem.textContent = 'RECONNECTING...';
    statusElem.parentElement.classList.remove('bg-emerald-500/10', 'border-emerald-500/30', 'text-emerald-400');
    statusElem.parentElement.classList.add('bg-rose-500/10', 'border-rose-500/30', 'text-rose-400');
  };
}

// 4. Update UI with incoming Telemetry
function updateDashboard(data) {
  // Header
  document.getElementById('server-host').textContent = `${data.system.hostname} • ${data.system.os_name} ${data.system.os_version}`;
  document.getElementById('uptime-display').textContent = data.system.uptime_human;

  // CPU
  document.getElementById('cpu-percent').textContent = `${data.cpu.total_usage_percent.toFixed(1)}%`;
  document.getElementById('cpu-mhz').textContent = `@ ${data.cpu.avg_frequency_mhz} MHz`;
  document.getElementById('cpu-bar').style.width = `${Math.min(data.cpu.total_usage_percent, 100)}%`;

  // Memory
  document.getElementById('mem-percent').textContent = `${data.memory.usage_percent.toFixed(1)}%`;
  const usedGb = (data.memory.used_bytes / (1024 ** 3)).toFixed(1);
  const totalGb = (data.memory.total_bytes / (1024 ** 3)).toFixed(1);
  document.getElementById('mem-human').textContent = `${usedGb} / ${totalGb} GB`;
  document.getElementById('mem-bar').style.width = `${Math.min(data.memory.usage_percent, 100)}%`;

  // Network
  document.getElementById('lan-rx').textContent = data.network.lan_rx_speed_human;
  document.getElementById('lan-tx').textContent = data.network.lan_tx_speed_human;
  document.getElementById('lan-total').textContent = `Total: ${data.network.lan_rx_total_human}`;

  document.getElementById('vpn-rx').textContent = data.network.vpn_rx_speed_human;
  document.getElementById('vpn-tx').textContent = data.network.vpn_tx_speed_human;
  document.getElementById('vpn-total').textContent = `Total: ${data.network.vpn_rx_total_human}`;

  // Gauges
  const fanPct = Math.min((data.thermals.fan_rpm / 5000.0) * 100.0, 100);
  fanGauge.updateSeries([Math.round(fanPct)]);

  const temp = data.thermals.cpu_temp_celsius;
  tempGauge.updateSeries([Math.round(temp)]);
  const tempStatus = document.getElementById('temp-status');
  if (temp < 55) {
    tempStatus.textContent = 'Cool & Optimal';
    tempStatus.className = 'text-emerald-400 font-semibold';
  } else if (temp < 75) {
    tempStatus.textContent = 'Warm';
    tempStatus.className = 'text-amber-400 font-semibold';
  } else {
    tempStatus.textContent = 'High Temp';
    tempStatus.className = 'text-rose-400 font-semibold';
  }

  const watts = data.power.current_watts;
  const powerScale = Math.min((watts / 10.0) * 100.0, 100);
  powerGauge.updateSeries([Math.round(watts * 10)]);
  document.getElementById('cost-month').textContent = data.power.estimated_monthly_cost;
  document.getElementById('cost-year').textContent = data.power.estimated_annual_cost;

  // Real-Time Streaming Chart
  if (isLiveStreaming) {
    const timestamp = data.timestamp * 1000;
    rollingHistory.push({
      timestamp,
      cpu: data.cpu.total_usage_percent,
      power: data.power.current_watts,
      temp: data.thermals.cpu_temp_celsius
    });

    if (rollingHistory.length > MAX_LIVE_POINTS) {
      rollingHistory.shift();
    }

    mainChart.updateSeries([
      { name: 'CPU Load (%)', data: rollingHistory.map(p => [p.timestamp, p.cpu]) },
      { name: 'Power Draw (W)', data: rollingHistory.map(p => [p.timestamp, p.power]) },
      { name: 'CPU Temp (°C)', data: rollingHistory.map(p => [p.timestamp, p.temp]) }
    ]);
  }

  // Docker Containers
  updateDockerContainers(data.containers);
}

// 5. Render Docker Containers
function updateDockerContainers(containers) {
  document.getElementById('container-count').textContent = containers.length;
  const grid = document.getElementById('docker-container-grid');
  grid.innerHTML = '';

  if (containers.length === 0) {
    grid.innerHTML = `
      <div class="col-span-2 text-center text-xs text-slate-500 py-6">
        No active Docker containers detected on /var/run/docker.sock
      </div>
    `;
    return;
  }

  containers.forEach(c => {
    const isRunning = c.is_running;
    const card = document.createElement('div');
    card.className = 'p-3 rounded-xl bg-slate-900/60 border border-slate-800 flex items-center justify-between hover:border-slate-700 transition';
    card.innerHTML = `
      <div class="flex items-center gap-3 overflow-hidden">
        <span class="w-2.5 h-2.5 rounded-full ${isRunning ? 'bg-emerald-400 shadow-sm shadow-emerald-400/50' : 'bg-rose-500'}"></span>
        <div class="truncate">
          <h4 class="text-xs font-bold text-slate-200 truncate">${c.name}</h4>
          <p class="text-[10px] text-slate-400 truncate font-mono">${c.image}</p>
        </div>
      </div>
      <span class="text-[10px] px-2 py-0.5 rounded font-mono font-medium ${isRunning ? 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20' : 'bg-rose-500/10 text-rose-400 border border-rose-500/20'}">
        ${c.status}
      </span>
    `;
    grid.appendChild(card);
  });
}

// 6. Load Config & Apps
async function loadConfig() {
  try {
    const res = await fetch('/api/config');
    const cfg = await res.json();
    const container = document.getElementById('app-cards-container');
    container.innerHTML = '';

    (cfg.apps || []).forEach(app => {
      // Smart dynamic host resolution for local & remote VPN
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
      tile.className = 'p-3 rounded-xl bg-slate-900/60 border border-slate-800 flex items-center justify-between hover:border-brand-500/50 hover:bg-slate-800/80 transition group';
      tile.innerHTML = `
        <div class="flex items-center gap-3">
          <div class="p-2 rounded-lg bg-brand-500/10 text-brand-400 group-hover:bg-brand-500 group-hover:text-white transition">
            <i data-lucide="${app.icon || 'globe'}" class="w-4 h-4"></i>
          </div>
          <div>
            <h4 class="text-xs font-bold text-white group-hover:text-brand-300 transition">${app.name}</h4>
            <p class="text-[10px] text-slate-400">${app.description}</p>
          </div>
        </div>
        <i data-lucide="external-link" class="w-3.5 h-3.5 text-slate-500 group-hover:text-brand-400 transition"></i>
      `;
      container.appendChild(tile);
    });

    lucide.createIcons();
  } catch (e) {
    console.error('Failed to load config:', e);
  }
}

// 7. Time-Range History Switching
function setupRangeButtons() {
  const buttons = document.querySelectorAll('.range-btn');
  buttons.forEach(btn => {
    btn.addEventListener('click', async () => {
      buttons.forEach(b => {
        b.classList.remove('bg-brand-500', 'text-white', 'font-semibold', 'shadow');
        b.classList.add('text-slate-400');
      });
      btn.classList.add('bg-brand-500', 'text-white', 'font-semibold', 'shadow');
      btn.classList.remove('text-slate-400');

      const seconds = parseInt(btn.dataset.seconds, 10);
      if (seconds === 60) {
        isLiveStreaming = true;
      } else {
        isLiveStreaming = false;
        await fetchHistoricalData(seconds);
      }
    });
  });
}

async function fetchHistoricalData(seconds) {
  try {
    const res = await fetch(`/api/history?seconds=${seconds}`);
    const points = await res.json();
    if (!points || points.length === 0) return;

    mainChart.updateSeries([
      { name: 'CPU Load (%)', data: points.map(p => [p.timestamp * 1000, p.cpu_usage]) },
      { name: 'Power Draw (W)', data: points.map(p => [p.timestamp * 1000, p.power_watts]) },
      { name: 'CPU Temp (°C)', data: points.map(p => [p.timestamp * 1000, p.cpu_temp]) }
    ]);
  } catch (e) {
    console.error('Failed to fetch history:', e);
  }
}
