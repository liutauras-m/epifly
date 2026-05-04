// ConusAI · Foundry client.
import { getActiveNodeId, isActiveConversationSelected } from '/assets/js/workspace.js';

// ── Theme toggle ───────────────────────────────────────────────────────────
const html = document.documentElement;
const themeBtn = document.getElementById('theme-toggle');
const themeIcon = document.getElementById('theme-icon');

function setTheme(t) {
  html.setAttribute('data-theme', t);
  localStorage.setItem('conusai-theme', t);
  if (themeIcon) themeIcon.setAttribute('href', t === 'forge' ? '/assets/icons/icons.svg#i-sun' : '/assets/icons/icons.svg#i-moon');
  if (themeBtn) themeBtn.setAttribute('aria-pressed', t === 'forge' ? 'true' : 'false');
}
setTheme(localStorage.getItem('conusai-theme') || 'paper');
themeBtn?.addEventListener('click', () => {
  const cur = html.getAttribute('data-theme') || 'paper';
  setTheme(cur === 'paper' ? 'forge' : 'paper');
});

// ── Chat state ────────────────────────────────────────────────────────────
const greetingScreen = document.getElementById('greeting-screen');
const chatView = document.getElementById('chat-view');
const messagesEl = document.getElementById('messages');
let activeThreadId = null;
let inFlight = false;

function showChatView() {
  if (!chatView?.hasAttribute('hidden')) return;
  greetingScreen?.setAttribute('hidden', '');
  chatView?.removeAttribute('hidden');
  chatView?.classList.remove('view-fade-in');
  void chatView?.offsetWidth; // reflow
  chatView?.classList.add('view-fade-in');
}
function showGreeting() {
  chatView?.setAttribute('hidden', '');
  greetingScreen?.removeAttribute('hidden');
  if (messagesEl) messagesEl.innerHTML = '';
  activeThreadId = null;
}

function ensureConversationSelected() {
  if (isActiveConversationSelected()) return true;
  window.__toast?.('Select or create a conversation (.md) before chatting', 'info');
  document.querySelector('[data-action="ws-new"]')?.click();
  return false;
}

// ── Per-node thread switching ─────────────────────────────────────────────
// Each conversation node owns its own persistent thread. Selecting a node
// swaps the active thread and rehydrates its message history. If the node
// has no binding yet (metadata.thread_id absent) we leave activeThreadId null;
// the server will create + bind on the first message of the turn.
async function loadThreadHistory(threadId) {
  if (!threadId) return;
  try {
    const res = await fetch(`/v1/threads/${threadId}/messages`);
    if (!res.ok) return;
    const { data } = await res.json();
    for (const m of data) {
      const role = m.role === 'assistant' ? 'ai' : m.role;
      appendMessage(role, m.content);
    }
    messagesEl.scrollTop = messagesEl.scrollHeight;
  } catch (_) {}
}

document.addEventListener('ws:select', async (e) => {
  if (inFlight) return;
  const { threadId } = e.detail || {};
  activeThreadId = threadId || null;
  if (messagesEl) messagesEl.innerHTML = '';
  if (threadId) {
    showChatView();
    await loadThreadHistory(threadId);
  } else {
    // Fresh node, no binding yet — show empty chat surface ready for first turn.
    showChatView();
  }
});

function nearBottom() {
  const m = messagesEl;
  return m.scrollHeight - m.scrollTop - m.clientHeight < 120;
}
function scrollIfNear() {
  if (nearBottom()) messagesEl.scrollTop = messagesEl.scrollHeight;
}

function appendMessage(role, text) {
  const el = document.createElement('div');
  el.className = `message ${role}`;
  if (role === 'ai') {
    const span = document.createElement('span');
    span.className = 'ai-text';
    span.textContent = text;
    el.appendChild(span);
  } else {
    el.textContent = text;
  }
  messagesEl.appendChild(el);
  scrollIfNear();
  return el;
}

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
}

function appendToolCard(name) {
  const [cap, tool] = name.includes('__') ? name.split('__', 2) : ['', name];
  const el = document.createElement('details');
  el.className = 'tool-card';
  el.dataset.status = 'running';
  el.dataset.startTime = String(performance.now());
  el.open = false;
  el.innerHTML = `
    <summary class="tool-head">
      <span class="tool-dot" role="status" aria-label="running"></span>
      ${cap ? `<span class="tool-cap">${escapeHtml(cap)}</span>` : ''}
      <span class="tool-glyph">·</span>
      <span class="tool-name">${escapeHtml(tool)}</span>
      <span class="tool-time" data-time>…</span>
    </summary>
    <div class="tool-body" data-body>running…</div>
  `;
  messagesEl.appendChild(el);
  el.scrollIntoView({ behavior: 'smooth', block: 'end' });
  return el;
}

function finalizeToolCard(card, resultText) {
  if (!card) return;
  let parsed = resultText;
  let isError = false;
  try {
    const obj = JSON.parse(resultText);
    parsed = JSON.stringify(obj, null, 2);
    if (obj && (obj.error || obj.status === 'error')) isError = true;
  } catch {}
  if (typeof resultText === 'string' && resultText.startsWith('Error:')) isError = true;
  card.dataset.status = isError ? 'error' : 'success';
  const statusDot = card.querySelector('[role="status"]');
  if (statusDot) statusDot.setAttribute('aria-label', isError ? 'error' : 'complete');
  const time = card.querySelector('[data-time]');
  if (time) {
    const ms = Math.round(performance.now() - Number(card.dataset.startTime || 0));
    time.textContent = ms < 1000 ? `${ms}ms` : `${(ms / 1000).toFixed(2)}s`;
  }
  const body = card.querySelector('[data-body]');
  if (body) {
    const trimmed = (parsed || '').slice(0, 2000);
    body.textContent = trimmed + ((parsed || '').length > 2000 ? '\n…' : '');
  }
}

async function streamChat(prompt) {
  if (inFlight) return;
  inFlight = true;
  showChatView();
  appendMessage('user', prompt);

  let aiEl = null;
  // Show thinking indicator until first token / tool event arrives
  let cursorEl = null;
  let thinkingEl = null;
  const showThinking = () => {
    if (thinkingEl) return;
    thinkingEl = document.createElement('div');
    thinkingEl.className = 'message ai thinking';
    thinkingEl.innerHTML = '<span class="thinking-dots" aria-label="Thinking"><i></i><i></i><i></i></span>';
    messagesEl.appendChild(thinkingEl);
    scrollIfNear();
  };
  const clearThinking = () => {
    if (thinkingEl) { thinkingEl.remove(); thinkingEl = null; }
  };
  const ensureAi = () => {
    clearThinking();
    if (!aiEl || !aiEl.isConnected || aiEl !== messagesEl.lastElementChild) {
      if (cursorEl) cursorEl.remove();
      aiEl = appendMessage('ai', '');
      aiEl.classList.add('streaming');
      cursorEl = document.createElement('span');
      cursorEl.className = 'cursor';
      cursorEl.setAttribute('aria-hidden', 'true');
      aiEl.appendChild(cursorEl);
    }
    return aiEl;
  };
  const sealAi = () => {
    if (cursorEl) { cursorEl.remove(); cursorEl = null; }
    if (aiEl) aiEl.classList.remove('streaming');
    aiEl = null;
  };

  const toolCardById = new Map();
  const toolNameById = new Map();

  try {
    showThinking();
    const res = await fetch('/ui/stream', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ message: prompt, thread_id: activeThreadId, workspace_node_id: getActiveNodeId() }),
    });
    if (!res.ok || !res.body) {
      const el = ensureAi();
      el.querySelector('.ai-text').textContent = `Error: ${res.status} ${res.statusText}`;
      el.classList.add('error');
      window.__toast?.(`Stream failed (${res.status})`, 'error');
      return;
    }

    const reader = res.body.getReader();
    const decoder = new TextDecoder();
    let buf = '';

    while (true) {
      const { value, done } = await reader.read();
      if (done) break;
      buf += decoder.decode(value, { stream: true });

      let pos;
      while ((pos = buf.indexOf('\n\n')) !== -1) {
        const block = buf.slice(0, pos);
        buf = buf.slice(pos + 2);
        for (const line of block.split('\n')) {
          if (!line.startsWith('data: ')) continue;
          const data = line.slice(6);
          if (data === '[DONE]') continue;
          let ev;
          try { ev = JSON.parse(data); } catch { continue; }
          const choice = ev.choices?.[0];
          const delta = choice?.delta;
          if (!delta) continue;
          if (typeof delta.content === 'string') {
            const el = ensureAi();
            el.querySelector('.ai-text').textContent += delta.content;
            if (cursorEl) el.appendChild(cursorEl); // keep cursor at end
            scrollIfNear();
          } else if (delta.tool_call_start) {
            clearThinking();
            sealAi();
            const { id, name } = delta.tool_call_start;
            toolCardById.set(id, appendToolCard(name));
            toolNameById.set(id, name);
          } else if (delta.tool_call_result) {
            const { tool_use_id, result } = delta.tool_call_result;
            finalizeToolCard(toolCardById.get(tool_use_id), result);
            // If this is invoice extraction, render the structured card
            const toolName = toolNameById.get(tool_use_id);
            if (toolName === 'invoice-processing__extract_invoice') {
              try {
                const data = JSON.parse(result);
                if (data && !data.error && data.invoice_number !== undefined) {
                  sealAi();
                  messagesEl.appendChild(renderInvoiceCard(data, 'invoice'));
                  scrollIfNear();
                }
              } catch {}
            }
          }
          if (ev.thread_id) activeThreadId = ev.thread_id;
        }
      }
    }
  } catch (e) {
    const el = ensureAi();
    el.querySelector('.ai-text').textContent = `Stream failed: ${e.message}`;
    el.classList.add('error');
    window.__toast?.(`Stream failed: ${e.message}`, 'error');
  } finally {
    clearThinking();
    sealAi();
    inFlight = false;
  }
}

// ── Invoice extraction (direct pipeline, no agent loop) ───────────────────
function renderInvoiceCard(data, filename) {
  const el = document.createElement('div');
  el.className = 'message ai invoice-result';

  const fmt = v => (v == null ? '—' : String(v));
  const fmtMoney = (v, cur) => v == null ? '—' : `${cur ?? ''}${Number(v).toFixed(2)}`;
  const cur = data.currency ?? '';

  const badge = data.status
    ? `<span class="inv-badge inv-badge-${(data.status || '').toLowerCase()}">${escapeHtml(data.status)}</span>`
    : '';

  const lineItems = (data.line_items ?? []).map(li => `
    <tr>
      <td>${escapeHtml(li.description ?? '')}</td>
      <td class="inv-num">${fmt(li.quantity)}</td>
      <td class="inv-num">${fmtMoney(li.unit_price, cur)}</td>
      <td class="inv-num">${fmtMoney(li.total, cur)}</td>
    </tr>`).join('');

  el.innerHTML = `
    <div class="inv-card">
      <div class="inv-header">
        <div class="inv-title-row">
          <span class="inv-label">Invoice</span>
          <strong class="inv-number">${escapeHtml(fmt(data.invoice_number))}</strong>
          ${badge}
        </div>
        <div class="inv-meta">
          ${data.invoice_date ? `<span>Date: <b>${escapeHtml(data.invoice_date)}</b></span>` : ''}
          ${data.due_date ? `<span>Due: <b>${escapeHtml(data.due_date)}</b></span>` : ''}
          ${data.order_number ? `<span>Order: <b>${escapeHtml(data.order_number)}</b></span>` : ''}
        </div>
      </div>

      <div class="inv-parties">
        <div class="inv-party">
          <div class="inv-party-label">From</div>
          <div class="inv-party-name">${escapeHtml(fmt(data.issuer_name))}</div>
          ${data.issuer_address ? `<div class="inv-party-detail">${escapeHtml(data.issuer_address)}</div>` : ''}
          ${data.issuer_vat ? `<div class="inv-party-detail">VAT: ${escapeHtml(data.issuer_vat)}</div>` : ''}
        </div>
        <div class="inv-party">
          <div class="inv-party-label">To</div>
          <div class="inv-party-name">${escapeHtml(fmt(data.billed_to_name))}</div>
          ${data.billed_to_company ? `<div class="inv-party-detail">${escapeHtml(data.billed_to_company)}</div>` : ''}
          ${data.billed_to_address ? `<div class="inv-party-detail">${escapeHtml(data.billed_to_address)}</div>` : ''}
          ${data.billed_to_email ? `<div class="inv-party-detail">${escapeHtml(data.billed_to_email)}</div>` : ''}
        </div>
      </div>

      ${lineItems ? `
      <table class="inv-table">
        <thead><tr><th>Description</th><th>Qty</th><th>Unit Price</th><th>Total</th></tr></thead>
        <tbody>${lineItems}</tbody>
      </table>` : ''}

      <div class="inv-totals">
        ${data.subtotal != null ? `<div class="inv-total-row"><span>Subtotal</span><span>${fmtMoney(data.subtotal, cur)}</span></div>` : ''}
        ${data.tax_amount != null ? `<div class="inv-total-row"><span>Tax</span><span>${fmtMoney(data.tax_amount, cur)}</span></div>` : ''}
        <div class="inv-total-row inv-grand-total"><span>Total</span><span>${fmtMoney(data.total_amount, cur)}</span></div>
        ${data.amount_due != null ? `<div class="inv-total-row"><span>Amount Due</span><span>${fmtMoney(data.amount_due, cur)}</span></div>` : ''}
      </div>

      ${data.notes ? `<div class="inv-notes">${escapeHtml(data.notes)}</div>` : ''}

      <div class="inv-source">Extracted from ${escapeHtml(filename)} via InvoicePipeline</div>
    </div>
  `;
  return el;
}

async function extractInvoice(token, filename) {
  if (inFlight) return;
  inFlight = true;
  showChatView();
  appendMessage('user', `Extract invoice data from ${filename}`);

  const loading = appendMessage('ai', '');
  loading.querySelector('.ai-text').textContent = 'Running invoice pipeline…';
  loading.classList.add('streaming');

  try {
    const res = await fetch('/ui/extract-invoice', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ token }),
    });
    loading.remove();
    if (!res.ok) {
      const err = await res.json().catch(() => ({ error: res.statusText }));
      const el = appendMessage('ai', '');
      el.querySelector('.ai-text').textContent = `Extraction failed: ${err.error ?? res.statusText}`;
      el.classList.add('error');
      window.__toast?.(`Invoice extraction failed`, 'error');
      return;
    }
    const data = await res.json();
    messagesEl.appendChild(renderInvoiceCard(data, filename));
    messagesEl.scrollTop = messagesEl.scrollHeight;
    window.__toast?.('Invoice extracted', 'info');
  } catch (e) {
    loading.remove();
    const el = appendMessage('ai', '');
    el.querySelector('.ai-text').textContent = `Error: ${e.message}`;
    el.classList.add('error');
    window.__toast?.(`Error: ${e.message}`, 'error');
  } finally {
    inFlight = false;
  }
}

// ── Attachments state ─────────────────────────────────────────────────────
const pendingAttachments = []; // [{id, filename, size}]

function fmtSize(n) {
  if (n < 1024) return `${n}B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)}KB`;
  return `${(n / (1024 * 1024)).toFixed(1)}MB`;
}

const INVOICE_EXTS = /\.(png|jpg|jpeg|pdf)$/i;
const INVOICE_NAMES = /invoice|receipt|bill|facture/i;

function isInvoiceFile(a) {
  return INVOICE_EXTS.test(a.filename) && INVOICE_NAMES.test(a.filename);
}

function renderAttachments() {
  document.querySelectorAll('[data-attachments]').forEach((el) => {
    el.innerHTML = '';
    if (pendingAttachments.length === 0) {
      el.classList.remove('has-items');
      return;
    }
    el.classList.add('has-items');
    for (const a of pendingAttachments) {
      const chip = document.createElement('span');
      chip.className = 'attachment';
      chip.innerHTML = `
        <span class="attachment-thumb"><svg class="icon"><use href="/assets/icons/icons.svg#i-file"/></svg></span>
        <span class="attachment-name">${escapeHtml(a.filename)}</span>
        <span class="attachment-size">${fmtSize(a.size)}</span>
        ${isInvoiceFile(a) ? `<button type="button" class="attachment-extract" title="Extract invoice data directly (no AI chat)">Extract invoice</button>` : ''}
        <button type="button" class="attachment-remove" aria-label="Remove">
          <svg class="icon"><use href="/assets/icons/icons.svg#i-x"/></svg>
        </button>
      `;
      chip.querySelector('.attachment-remove').addEventListener('click', () => {
        const i = pendingAttachments.indexOf(a);
        if (i >= 0) pendingAttachments.splice(i, 1);
        renderAttachments();
      });
      chip.querySelector('.attachment-extract')?.addEventListener('click', () => {
        const i = pendingAttachments.indexOf(a);
        if (i >= 0) pendingAttachments.splice(i, 1);
        renderAttachments();
        extractInvoice(a.id, a.filename);
      });
      el.appendChild(chip);
    }
  });
}

async function uploadFiles(files) {
  for (const file of files) {
    const fd = new FormData();
    fd.append('file', file, file.name);
    try {
      const res = await fetch('/ui/upload', { method: 'POST', body: fd });
      if (!res.ok) {
        console.warn('[upload] failed', res.status);
        continue;
      }
      const data = await res.json();
      pendingAttachments.push({
        id: data.id,
        filename: data.filename,
        size: data.size,
      });
      renderAttachments();
    } catch (e) {
      console.warn('[upload] error', e);
    }
  }
}

// ── Composer wiring ───────────────────────────────────────────────────────
document.querySelectorAll('[data-composer]').forEach((form) => {
  const ta = form.querySelector('[data-input]');
  if (!ta) return;
  const grow = () => {
    ta.style.height = 'auto';
    ta.style.height = Math.min(ta.scrollHeight, 240) + 'px';
  };
  ta.addEventListener('input', grow);
  ta.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      form.requestSubmit();
    }
  });

  // Paperclip → file picker
  const fileInput = form.querySelector('[data-file-input]');
  const attachBtn = form.querySelector('[data-attach]');
  attachBtn?.addEventListener('click', () => fileInput?.click());
  fileInput?.addEventListener('change', () => {
    if (fileInput.files?.length) uploadFiles([...fileInput.files]);
    fileInput.value = '';
  });

  // Drag & drop on the composer
  form.addEventListener('dragover', (e) => {
    if (e.dataTransfer?.types?.includes('Files')) {
      e.preventDefault();
      form.classList.add('drop-target');
    }
  });
  form.addEventListener('dragleave', () => form.classList.remove('drop-target'));
  form.addEventListener('drop', (e) => {
    e.preventDefault();
    form.classList.remove('drop-target');
    if (e.dataTransfer?.files?.length) uploadFiles([...e.dataTransfer.files]);
  });

  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const val = ta.value.trim();
    if (!val && pendingAttachments.length === 0) return;
    if (!ensureConversationSelected()) return;
    ta.value = '';
    grow();
    let prompt = val;
    if (pendingAttachments.length) {
      const origin = window.location.origin;
      const lines = pendingAttachments
        .map(a => `- ${a.filename} (image_path: ${origin}/v1/files/${a.id})`)
        .join('\n');
      prompt = `${val}\n\n[Attached files — pass image_path directly to invoice-processing__extract_invoice or ocr-service__extract_text]\n${lines}`;
    }
    pendingAttachments.length = 0;
    renderAttachments();
    streamChat(prompt);
  });
});

// ── Quick chips ───────────────────────────────────────────────────────────
document.querySelectorAll('.chip').forEach((chip) => {
  chip.addEventListener('click', () => {
    const prompt = chip.dataset.prompt || '';
    const ta = document.querySelector('.greeting-screen [data-input]');
    if (!ta) return;
    ta.value = prompt;
    ta.focus();
    ta.setSelectionRange(prompt.length, prompt.length);
    ta.dispatchEvent(new Event('input'));
  });
});

// ── Recents → load thread history ────────────────────────────────────────
document.querySelectorAll('.recent').forEach((el) => {
  el.addEventListener('click', async () => {
    const threadId = el.dataset.threadId;
    if (!threadId || inFlight) return;
    showChatView();
    activeThreadId = threadId;
    messagesEl.innerHTML = '';
    const loading = appendMessage('ai', '');
    loading.querySelector('.ai-text').textContent = 'Loading…';
    try {
      const res = await fetch(`/v1/threads/${threadId}/messages`, {
        headers: { 'X-Tenant-ID': 'dev' },
      });
      loading.remove();
      if (!res.ok) { window.__toast?.('Could not load thread', 'error'); return; }
      const data = await res.json();
      const msgs = Array.isArray(data) ? data : (data.messages ?? []);
      if (msgs.length === 0) {
        const hint = appendMessage('ai', '');
        hint.querySelector('.ai-text').textContent = 'No messages in this thread yet.';
        return;
      }
      for (const m of msgs) {
        const b = appendMessage(m.role === 'user' ? 'user' : 'ai', '');
        if (m.role === 'user') b.textContent = m.content;
        else b.querySelector('.ai-text').textContent = m.content;
      }
    } catch (e) {
      loading.remove();
      window.__toast?.(`Failed: ${e.message}`, 'error');
    }
  });
});

// ── Capability click → @mention ───────────────────────────────────────────
document.querySelectorAll('.cap').forEach((el) => {
  el.addEventListener('click', () => {
    const name = el.dataset.cap || '';
    const ta = document.querySelector('.greeting-screen [data-input]')
      || document.querySelector('.composer-bottom [data-input]');
    if (!ta) return;
    ta.value = (ta.value ? ta.value + ' ' : '') + '@' + name + ' ';
    ta.focus();
    ta.dispatchEvent(new Event('input'));
  });
});

// ── Mobile sidebar toggle ─────────────────────────────────────────────────
const sidebarEl = document.querySelector('.sidebar');
const sidebarToggle = document.getElementById('sidebar-toggle');
const sidebarBackdrop = document.getElementById('sidebar-backdrop');
function openSidebar() {
  sidebarEl?.classList.add('open');
  sidebarToggle?.setAttribute('aria-expanded', 'true');
}
function closeSidebar() {
  sidebarEl?.classList.remove('open');
  sidebarToggle?.setAttribute('aria-expanded', 'false');
}
sidebarToggle?.addEventListener('click', () =>
  sidebarEl?.classList.contains('open') ? closeSidebar() : openSidebar()
);
sidebarBackdrop?.addEventListener('click', closeSidebar);

// ── New workspace item ────────────────────────────────────────────────────
function openNewWorkspaceItem() {
  const trigger = document.querySelector('[data-action="ws-new"]');
  if (trigger) {
    trigger.click();
    return;
  }
  window.__toast?.('Workspace create action is unavailable', 'error');
}

// ── Keyboard shortcuts ────────────────────────────────────────────────────
function focusActiveComposer() {
  const visible = !chatView?.hasAttribute('hidden')
    ? document.querySelector('.composer-bottom [data-input]')
    : document.querySelector('.greeting-screen [data-input]');
  visible?.focus();
}

document.addEventListener('keydown', (e) => {
  const mod = e.metaKey || e.ctrlKey;
  if (mod && e.key === 'k') {
    e.preventDefault();
    focusActiveComposer();
  } else if (mod && e.key === 'n') {
    e.preventDefault();
    openNewWorkspaceItem();
  } else if (mod && e.key === '/') {
    e.preventDefault();
    setTheme(html.getAttribute('data-theme') === 'paper' ? 'forge' : 'paper');
  } else if (e.key === 'Escape') {
    document.activeElement?.blur?.();
  }
});

// ── Toast ─────────────────────────────────────────────────────────────────
function toast(msg, kind = 'info') {
  let host = document.getElementById('toasts');
  if (!host) {
    host = document.createElement('div');
    host.id = 'toasts';
    host.className = 'toasts';
    document.body.appendChild(host);
  }
  const el = document.createElement('div');
  el.className = `toast toast-${kind}`;
  el.textContent = msg;
  host.appendChild(el);
  setTimeout(() => { el.classList.add('out'); setTimeout(() => el.remove(), 300); }, 3500);
}
// ── Recents — click a recent thread to reopen it ─────────────────────────
document.querySelectorAll('.recent[data-thread-id]').forEach((el) => {
  el.addEventListener('click', async () => {
    if (inFlight) return;
    const threadId = el.dataset.threadId;
    activeThreadId = threadId;
    // Deselect any active workspace node highlight
    document.querySelectorAll('[aria-current="page"]').forEach((n) => n.removeAttribute('aria-current'));
    if (messagesEl) messagesEl.innerHTML = '';
    showChatView();
    await loadThreadHistory(threadId);
  });
});

window.__toast = toast;
window.__extractInvoice = extractInvoice;
window.__pendingAttachments = pendingAttachments;
window.__renderAttachments = renderAttachments;
