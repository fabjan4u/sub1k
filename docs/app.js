// Sub-1K Radio Client-side Application
let latestData = null;
let historyData = null;
let currentFilter = 'all'; // 'all', playlist_id, or 'history'
let currentSearch = '';

document.addEventListener('DOMContentLoaded', () => {
  initApp();
});

async function initApp() {
  try {
    const res = await fetch('data/latest.json');
    if (!res.ok) throw new Error('Failed to load latest tracks');
    latestData = await res.json();
    
    // Update metric counters
    updateMetrics();

    // Render Initial Tracks Table
    renderTracks();

    // Setup Event Listeners
    setupEvents();
  } catch (err) {
    console.error('Error initializing Sub-1K portal:', err);
    document.getElementById('tracks-body').innerHTML = `
      <tr>
        <td colspan="6" class="loading-state" style="color: #ff3366;">
          ⚠️ Failed to load station data. If you are viewing locally without a web server, please use an HTTP server (e.g. npx serve docs).
        </td>
      </tr>
    `;
  }
}

function updateMetrics() {
  if (!latestData) return;

  const totalToday = latestData.latest_tracks ? latestData.latest_tracks.length : 100;
  const totalArchived = latestData.total_archived || 100;
  const playlistCount = latestData.playlists ? latestData.playlists.length : 2;

  document.getElementById('stat-playlists').textContent = playlistCount;
  document.getElementById('stat-tracks-today').textContent = totalToday;
  document.getElementById('stat-total-archived').textContent = `${totalArchived}+`;
}

async function loadHistory() {
  if (!historyData) {
    const res = await fetch('data/history.json');
    if (res.ok) {
      historyData = await res.json();
    } else {
      historyData = latestData ? latestData.latest_tracks : [];
    }
  }
  return historyData;
}

async function renderTracks() {
  const tbody = document.getElementById('tracks-body');
  const countIndicator = document.getElementById('track-count-indicator');
  
  let dataset = [];
  if (currentFilter === 'history') {
    tbody.innerHTML = `
      <tr>
        <td colspan="6" class="loading-state">
          <span class="pulse-dot"></span> Pulling full vault archive...
        </td>
      </tr>
    `;
    dataset = await loadHistory();
  } else {
    dataset = (latestData && latestData.latest_tracks) ? latestData.latest_tracks : [];
    if (currentFilter !== 'all') {
      dataset = dataset.filter(t => t.playlist_id === currentFilter);
    }
  }

  // Filter with Search query
  if (currentSearch.trim()) {
    const q = currentSearch.toLowerCase().trim();
    dataset = dataset.filter(t => 
      t.title.toLowerCase().includes(q) ||
      t.artist.toLowerCase().includes(q) ||
      t.album.toLowerCase().includes(q)
    );
  }

  countIndicator.textContent = `Showing ${dataset.length} track${dataset.length === 1 ? '' : 's'}`;

  if (dataset.length === 0) {
    tbody.innerHTML = `
      <tr>
        <td colspan="6" class="loading-state">
          No underground tracks found matching your query.
        </td>
      </tr>
    `;
    return;
  }

  // Generate Table Rows
  const rowsHtml = dataset.map((track, idx) => {
    const coverUrl = track.album_art || 'data:image/svg+xml,<svg xmlns="http://www.w3.org/2000/svg" width="44" height="44"><rect width="44" height="44" fill="%23181d28"/></svg>';
    const isAnchor = track.artist === (latestData ? latestData.anchor_artist : 'THE SILICON RITE');
    const badgeHtml = isAnchor ? '<span class="artist-anchor-badge">CURATOR / ANCHOR</span>' : '';
    const popDisplay = track.popularity !== undefined ? track.popularity : 0;
    
    // Clean playlist name badge
    const isDark = track.playlist_id === '1kmZVpw99RovDgeLYfHoPa';
    const plBadge = isDark 
      ? '<span class="badge" style="font-size:0.7rem; padding: 2px 8px;">Dark Synth</span>'
      : '<span class="badge badge-red" style="font-size:0.7rem; padding: 2px 8px;">Deathcore</span>';

    return `
      <tr>
        <td style="color: var(--text-dim); font-family: var(--font-mono); font-size: 0.85rem;">${idx + 1}</td>
        <td>
          <div class="track-col">
            <img class="track-cover" src="${escapeHtml(coverUrl)}" alt="Cover" loading="lazy" />
            <div class="track-info">
              <a href="${escapeHtml(track.spotify_url)}" target="_blank" rel="noopener" class="track-title">${escapeHtml(track.title)}</a>
              <span class="track-artist">${escapeHtml(track.artist)} ${badgeHtml}</span>
            </div>
          </div>
        </td>
        <td style="color: var(--text-muted); font-size: 0.85rem;">${escapeHtml(track.album || 'Single')}</td>
        <td>${plBadge}</td>
        <td>
          <span class="pop-indicator">
            <span class="pulse-dot" style="width: 6px; height: 6px; background-color: var(--accent-green);"></span>
            ${popDisplay} / 100
          </span>
        </td>
        <td>
          <a href="${escapeHtml(track.spotify_url)}" target="_blank" rel="noopener" class="btn btn-sm btn-spotify" style="padding: 4px 10px; font-size: 0.75rem;">
            Play ↗
          </a>
        </td>
      </tr>
    `;
  }).join('');

  tbody.innerHTML = rowsHtml;
}

function setupEvents() {
  // Search input
  const searchInput = document.getElementById('track-search');
  searchInput.addEventListener('input', (e) => {
    currentSearch = e.target.value;
    renderTracks();
  });

  // Filter Buttons
  const buttons = document.querySelectorAll('.filter-btn');
  buttons.forEach(btn => {
    btn.addEventListener('click', () => {
      buttons.forEach(b => b.classList.remove('active'));
      btn.classList.add('active');

      if (btn.dataset.view === 'history') {
        currentFilter = 'history';
      } else {
        currentFilter = btn.dataset.filter || 'all';
      }

      renderTracks();
    });
  });
}

function escapeHtml(str) {
  if (!str) return '';
  return String(str)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}
