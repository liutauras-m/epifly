/**
 * workspace.js — hierarchical workspace tree (folders + conversations).
 *
 * Responsibilities:
 *  - Load root nodes on page load; lazy-load children on folder expand.
 *  - "+" button → dialog for new folder/conversation.
 *  - Right-click context menu: New folder, New conversation, Share, Move, Delete.
 *  - Selecting a conversation sets the active workspace_node_id for all agent requests.
 *  - Live-save on editor blur (debounced).
 *  - Keyboard nav: ↑/↓ focus, → expand, ← collapse, Enter open, F2 rename, Delete key.
 */

const WS_API = "/v1/workspaces";
let activeNodeId = null;
let treeEl = null;
let emptyEl = null;

// ── Bootstrap ────────────────────────────────────────────────────────────────

document.addEventListener("DOMContentLoaded", async () => {
  treeEl = document.getElementById("workspace-tree");
  emptyEl = document.querySelector(".ws-empty");
  if (!treeEl) return;

  await loadTree(null, treeEl);

  // Restore selection from URL if present
  const initialNodeId = new URLSearchParams(window.location.search).get("ws");
  if (initialNodeId) {
    await restoreNodeFromUrl(initialNodeId);
  }

  document.querySelectorAll("[data-action='ws-new']").forEach((btn) =>
    btn.addEventListener("click", () => openNewDialog(activeParentFolder()))
  );

  document.addEventListener("keydown", onGlobalKey);
  document.addEventListener("click", dismissContextMenu);

  // Browser back/forward updates tree selection
  window.addEventListener("popstate", (e) => {
    const nodeId = e.state?.wsNodeId ?? new URLSearchParams(window.location.search).get("ws");
    if (nodeId) {
      restoreNodeFromUrl(nodeId);
    } else {
      treeEl.querySelectorAll("[aria-current='page']").forEach((n) => n.removeAttribute("aria-current"));
      activeNodeId = null;
    }
  });

  initSearch();
});

// ── Workspace search ──────────────────────────────────────────────────────────

function initSearch() {
  const input = document.getElementById("ws-search");
  const clearBtn = document.getElementById("ws-search-clear");
  if (!input) return;

  let debounceTimer = null;

  input.addEventListener("input", () => {
    clearTimeout(debounceTimer);
    const q = input.value.trim();
    clearBtn.hidden = q.length === 0;
    debounceTimer = setTimeout(() => applySearch(q), 220);
  });

  clearBtn.addEventListener("click", () => {
    input.value = "";
    clearBtn.hidden = true;
    applySearch("");
    input.focus();
  });
}

// ── Search results panel ──────────────────────────────────────────────────────

let searchResultsEl = null;

function getOrCreateResultsPanel() {
  if (searchResultsEl) return searchResultsEl;
  searchResultsEl = document.createElement("div");
  searchResultsEl.className = "ws-search-results";
  searchResultsEl.hidden = true;
  // Insert just after the search wrap, before the tree
  const searchWrap = document.querySelector(".ws-search-wrap");
  searchWrap?.insertAdjacentElement("afterend", searchResultsEl);
  return searchResultsEl;
}

async function applySearch(query) {
  if (!treeEl) return;
  const panel = getOrCreateResultsPanel();

  if (!query) {
    // Clear search state — restore normal tree
    treeEl.hidden = false;
    panel.hidden = true;
    panel.innerHTML = "";
    return;
  }

  // Hide the normal tree, show the results panel while loading
  treeEl.hidden = true;
  panel.hidden = false;
  panel.innerHTML = `<div class="ws-search-loading"><span class="ws-search-spinner"></span></div>`;

  let results = [];
  try {
    results = await apiFetch(`/v1/workspaces/search?q=${encodeURIComponent(query)}&limit=40`);
    if (!Array.isArray(results)) results = [];
  } catch (_) {
    // API unavailable — fall back to DOM name scan below
  }

  // If API returned nothing, do a local DOM name-match fallback
  if (results.length === 0) {
    const q = query.toLowerCase();
    results = collectAllTreeNodes(treeEl)
      .filter(({ nameEl }) => nameEl?.textContent.toLowerCase().includes(q))
      .map(({ el }) => ({
        id: el.dataset.id,
        name: el.querySelector(".ws-name")?.textContent || "",
        kind: el.classList.contains("ws-folder") ? "folder" : "conversation",
      }));
  }

  if (results.length === 0) {
    panel.innerHTML = `<div class="ws-search-empty">No results for <em>${escHtml(query)}</em></div>`;
    return;
  }

  // Render a flat list of matching nodes
  const q = query.toLowerCase();
  panel.innerHTML = "";
  for (const node of results) {
    const isFolder = node.kind === "folder";
    const div = document.createElement("div");
    div.className = "ws-search-hit";
    div.dataset.id = node.id;

    // Icon
    const iconSvg = isFolder
      ? `<svg class="icon" viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M2 5h5l2 2h7v9H2z"/></svg>`
      : `<svg class="icon" viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M4 2h7l4 4v11H4z"/><polyline points="11,2 11,6 15,6"/></svg>`;

    // Highlight matching chars in name
    const name = node.name || "";
    const idx = name.toLowerCase().indexOf(q);
    let highlighted = escHtml(name);
    if (idx !== -1) {
      highlighted =
        escHtml(name.slice(0, idx)) +
        `<mark>${escHtml(name.slice(idx, idx + q.length))}</mark>` +
        escHtml(name.slice(idx + q.length));
    }

    div.innerHTML = `${iconSvg}<span class="ws-name">${highlighted}</span>`;

    if (!isFolder) {
      div.addEventListener("click", () => {
        // Find or build a lightweight node descriptor and select it
        const existing = treeEl.querySelector(`[data-id="${node.id}"]`);
        if (existing) {
          selectConversation(node, existing);
        } else {
          // Node not in DOM — dispatch select directly with node data
          selectConversation(node, div);
        }
      });
    }

    panel.appendChild(div);
  }
}

// Walk the tree DOM and collect { el, nameEl } for every node element.
function collectAllTreeNodes(root) {
  const results = [];
  function walk(container) {
    for (const child of container.children) {
      if (child.classList.contains("ws-folder")) {
        const nameEl = child.querySelector(":scope > summary .ws-name");
        results.push({ el: child, nameEl });
        const childContainer = child.querySelector(":scope > .ws-children");
        if (childContainer) walk(childContainer);
      } else if (child.classList.contains("ws-conversation")) {
        const nameEl = child.querySelector(".ws-name");
        results.push({ el: child, nameEl });
      } else {
        walk(child);
      }
    }
  }
  walk(root);
  return results;
}

// ── Tree loading ──────────────────────────────────────────────────────────────

async function loadTree(parentId, containerEl) {
  containerEl.setAttribute("aria-busy", "true");
  const url = parentId
    ? `${WS_API}/tree?parent_id=${parentId}`
    : `${WS_API}/tree`;
  try {
    const nodes = await apiFetch(url);
    containerEl.innerHTML = "";
    containerEl.setAttribute("aria-busy", "false");
    if (!nodes.length) {
      if (!parentId && emptyEl) emptyEl.hidden = false;
      return;
    }
    if (!parentId && emptyEl) emptyEl.hidden = true;
    nodes.forEach((node) => containerEl.appendChild(buildNodeEl(node)));
  } catch (e) {
    containerEl.setAttribute("aria-busy", "false");
    showToast("Failed to load workspace: " + e.message, "error");
  }
}

// ── DOM builders ─────────────────────────────────────────────────────────────

function buildNodeEl(node) {
  if (node.kind === "folder") {
    return buildFolderEl(node);
  }
  return buildConversationEl(node);
}

function buildFolderEl(node) {
  const details = document.createElement("details");
  details.className = "ws-folder";
  details.dataset.id = node.id;

  const summary = document.createElement("summary");
  summary.setAttribute("role", "treeitem");
  summary.setAttribute("aria-expanded", "false");
  summary.tabIndex = 0;
  summary.innerHTML = `
    <svg class="icon chev" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5">
      <polyline points="3,2 9,6 3,10"/>
    </svg>
    <svg class="icon" viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.5">
      <path d="M2 5h5l2 2h7v9H2z"/>
    </svg>
    <span class="ws-name">${escHtml(node.name)}</span>`;
  summary.addEventListener("click", () => selectFolder(node, summary));
  summary.addEventListener("contextmenu", (e) => {
    e.preventDefault();
    showContextMenu(e, node);
  });
  summary.addEventListener("keydown", (e) => onNodeKey(e, node));

  details.addEventListener("toggle", async () => {
    summary.setAttribute("aria-expanded", details.open ? "true" : "false");
    if (details.open && details.children.length === 1) {
      const childContainer = document.createElement("div");
      childContainer.className = "ws-children";
      childContainer.setAttribute("role", "group");
      details.appendChild(childContainer);
      await loadTree(node.id, childContainer);
    }
  });

  details.appendChild(summary);
  return details;
}

function buildConversationEl(node) {
  const div = document.createElement("div");
  div.className = "ws-conversation";
  div.setAttribute("role", "treeitem");
  div.setAttribute("tabindex", "0");
  div.dataset.id = node.id;
  div.dataset.virtualPath = node.virtual_path;
  div.innerHTML = `
    <svg class="icon" viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.5">
      <rect x="3" y="2" width="12" height="14" rx="1"/>
      <line x1="6" y1="6" x2="12" y2="6"/>
      <line x1="6" y1="9" x2="12" y2="9"/>
      <line x1="6" y1="12" x2="9" y2="12"/>
    </svg>
    <span class="ws-name">${escHtml(node.name)}</span>`;

  div.addEventListener("click", () => selectConversation(node, div));
  div.addEventListener("contextmenu", (e) => {
    e.preventDefault();
    showContextMenu(e, node);
  });
  div.addEventListener("keydown", (e) => onNodeKey(e, node));
  return div;
}

// ── Selection ─────────────────────────────────────────────────────────────────

function selectFolder(node, summaryEl) {
  // Clear previous selection (conversations and folders)
  treeEl.querySelectorAll("[aria-current='page']").forEach((n) =>
    n.removeAttribute("aria-current")
  );
  summaryEl.setAttribute("aria-current", "page");
  activeNodeId = node.id;
}

async function selectConversation(node, el) {
  // Clear previous selection
  treeEl.querySelectorAll("[aria-current='page']").forEach((n) =>
    n.removeAttribute("aria-current")
  );
  el.setAttribute("aria-current", "page");
  activeNodeId = node.id;

  // Push URL state so the selection survives reload and enables deep-linking
  const url = new URL(window.location.href);
  url.searchParams.set("ws", node.id);
  history.pushState({ wsNodeId: node.id }, "", url.toString());

  // Ensure we have full metadata (restoreNodeFromUrl/click stubs may not include it).
  let full = node;
  if (!full.metadata) {
    try {
      full = await apiFetch(`${WS_API}/${node.id}`);
    } catch (_) {}
  }
  const threadId = full?.metadata?.thread_id ?? null;

  // Expose to app.js via a custom event — app.js owns thread loading & UI swap.
  document.dispatchEvent(
    new CustomEvent("ws:select", {
      detail: { nodeId: node.id, node: full, threadId },
    })
  );

  // Load content into editor if present
  try {
    const res = await apiFetch(`${WS_API}/${node.id}/content`);
    document.dispatchEvent(
      new CustomEvent("ws:content", {
        detail: { nodeId: node.id, content: res.content },
      })
    );
  } catch (_) {}
}

// ── URL state restoration ─────────────────────────────────────────────────────

async function restoreNodeFromUrl(nodeId) {
  // Try to find the element already rendered in the tree
  let el = treeEl.querySelector(`[data-id="${nodeId}"]`);
  if (el) {
    selectConversation({ id: nodeId, kind: "conversation" }, el);
    return;
  }
  // Node may be inside a collapsed folder — fetch node metadata then expand ancestors
  try {
    const node = await apiFetch(`${WS_API}/${nodeId}`);
    if (node.kind !== "conversation") return;
    // Expand folder ancestors in order. Opening a <details> fires the toggle handler
    // which lazy-loads children; wait for aria-busy to clear before proceeding.
    const pathParts = node.virtual_path.split("/").slice(0, -1);
    let container = treeEl;
    for (const part of pathParts) {
      const folderEl = Array.from(container.querySelectorAll(".ws-folder")).find(
        (d) => d.querySelector(".ws-name")?.textContent === part
      );
      if (!folderEl) break;
      if (!folderEl.open) {
        // Toggle the <details> open — the toggle handler will create .ws-children and call loadTree
        folderEl.open = true;
        // Wait for the lazy-load started by the toggle handler to finish
        await waitForLoad(folderEl);
      }
      container = folderEl;
    }
    el = treeEl.querySelector(`[data-id="${nodeId}"]`);
    if (el) selectConversation(node, el);
  } catch (_) {}
}

// Poll until aria-busy is gone from the folder's children container.
function waitForLoad(folderEl) {
  return new Promise((resolve) => {
    const check = () => {
      const children = folderEl.querySelector(".ws-children");
      if (!children || children.getAttribute("aria-busy") !== "true") {
        resolve();
      } else {
        requestAnimationFrame(check);
      }
    };
    requestAnimationFrame(check);
  });
}

// ── New node dialog ───────────────────────────────────────────────────────────

function openNewDialog(parentNode, defaultKind = "folder") {
  const existing = document.getElementById("ws-new-dialog");
  if (existing) existing.remove();

  const dialog = document.createElement("dialog");
  dialog.id = "ws-new-dialog";
  dialog.setAttribute("aria-labelledby", "ws-dialog-title");
  dialog.innerHTML = `
    <form method="dialog" class="ws-dialog-form">
      <h2 id="ws-dialog-title" class="ws-dialog-title">New item</h2>
      <fieldset class="ws-kind-group">
        <legend class="t-label">Type</legend>
        <label class="ws-radio"><input type="radio" name="kind" value="folder"${defaultKind === "folder" ? " checked" : ""}> Folder</label>
        <label class="ws-radio"><input type="radio" name="kind" value="conversation"${defaultKind === "conversation" ? " checked" : ""}> Conversation (.md)</label>
      </fieldset>
      <label class="ws-field">
        <span class="t-label">Name</span>
        <input id="ws-name-input" type="text" class="ws-input"
          placeholder="${defaultKind === "conversation" ? "New conversation.md" : "My folder"}"
          required maxlength="255" autocomplete="off">
      </label>
      ${parentNode ? `<p class="ws-dialog-hint">Inside: <strong>${escHtml(parentNode.name)}</strong></p>` : ""}
      <div class="ws-dialog-actions">
        <button type="button" class="btn btn-ghost" id="ws-cancel">Cancel</button>
        <button type="submit" class="btn btn-primary">Create</button>
      </div>
    </form>`;

  document.body.appendChild(dialog);

  const kindInputs = dialog.querySelectorAll("input[name='kind']");
  const nameInput = dialog.querySelector("#ws-name-input");

  kindInputs.forEach((r) =>
    r.addEventListener("change", () => {
      if (r.value === "conversation" && !nameInput.value.endsWith(".md")) {
        nameInput.value = nameInput.value
          ? nameInput.value + ".md"
          : "New conversation.md";
        nameInput.placeholder = "New conversation.md";
      } else if (r.value === "folder") {
        nameInput.placeholder = "My folder";
      }
    })
  );

  dialog.querySelector("#ws-cancel").addEventListener("click", () =>
    dialog.close()
  );

  dialog.querySelector("form").addEventListener("submit", async (e) => {
    e.preventDefault();
    const kind = dialog.querySelector("input[name='kind']:checked").value;
    let name = nameInput.value.trim();
    if (kind === "conversation" && !name.endsWith(".md")) name += ".md";

    try {
      const body = {
        kind,
        name,
        parent_id: parentNode ? parentNode.id : null,
      };
      await apiFetch(WS_API, { method: "POST", body: JSON.stringify(body) });
      dialog.close();
      refreshParent(parentNode);
    } catch (err) {
      showToast("Create failed: " + err.message, "error");
    }
  });

  dialog.showModal();
  nameInput.focus();
}

// ── Context menu ──────────────────────────────────────────────────────────────

function showContextMenu(event, node) {
  dismissContextMenu();

  const menu = document.createElement("ul");
  menu.id = "ws-ctx-menu";
  menu.setAttribute("role", "menu");
  menu.className = "ws-context-menu";
  menu.style.left = event.clientX + "px";
  menu.style.top = event.clientY + "px";

  const items =
    node.kind === "folder"
      ? [
          { label: "New folder", action: () => openNewDialog(node, "folder") },
          { label: "New conversation", action: () => openNewDialog(node, "conversation") },
          { label: "Share…", action: () => openShareDialog(node) },
          { label: "Delete", action: () => confirmDelete(node), cls: "danger" },
        ]
      : [
          { label: "Share…", action: () => openShareDialog(node) },
          { label: "Move…", action: () => openMoveDialog(node) },
          { label: "Delete", action: () => confirmDelete(node), cls: "danger" },
        ];

  items.forEach(({ label, action, cls }) => {
    const li = document.createElement("li");
    li.setAttribute("role", "menuitem");
    li.tabIndex = -1;
    li.textContent = label;
    if (cls) li.className = cls;
    li.addEventListener("click", () => {
      dismissContextMenu();
      action();
    });
    menu.appendChild(li);
  });

  document.body.appendChild(menu);
  menu.querySelector("[role='menuitem']")?.focus();
}

function dismissContextMenu() {
  document.getElementById("ws-ctx-menu")?.remove();
}

// ── Share dialog ───────────────────────────────────────────────────────────────

function openShareDialog(node) {
  const existing = document.getElementById("ws-share-dialog");
  if (existing) existing.remove();

  const dialog = document.createElement("dialog");
  dialog.id = "ws-share-dialog";
  dialog.setAttribute("aria-labelledby", "ws-share-title");
  dialog.innerHTML = `
    <div class="ws-dialog-form">
      <h2 id="ws-share-title" class="ws-dialog-title">Share "${escHtml(node.name)}"</h2>
      <div class="ws-shared-list" id="ws-shared-list">
        ${(node.shared_with || []).map((uid) => `
          <div class="ws-shared-row">
            <span>${escHtml(uid)}</span>
            <button type="button" class="btn btn-ghost btn-sm" data-unshare="${escHtml(uid)}">Remove</button>
          </div>`).join("") || '<p class="ws-dialog-hint">Not shared with anyone.</p>'}
      </div>
      <label class="ws-field">
        <span class="t-label">Add user ID</span>
        <input id="ws-share-input" type="text" class="ws-input" placeholder="user-abc123" autocomplete="off">
      </label>
      <div class="ws-dialog-actions">
        <button type="button" class="btn btn-ghost" id="ws-share-cancel">Close</button>
        <button type="button" class="btn btn-primary" id="ws-share-add">Share</button>
      </div>
    </div>`;

  document.body.appendChild(dialog);

  dialog.querySelectorAll("[data-unshare]").forEach((btn) =>
    btn.addEventListener("click", async () => {
      try {
        const updated = await apiFetch(`${WS_API}/${node.id}/unshare`, {
          method: "POST",
          body: JSON.stringify({ user_id: btn.dataset.unshare }),
        });
        Object.assign(node, updated);
        openShareDialog(node); // re-open refreshed
        dialog.close();
      } catch (e) {
        showToast("Unshare failed: " + e.message, "error");
      }
    })
  );

  dialog.querySelector("#ws-share-cancel").addEventListener("click", () =>
    dialog.close()
  );
  dialog.querySelector("#ws-share-add").addEventListener("click", async () => {
    const uid = dialog.querySelector("#ws-share-input").value.trim();
    if (!uid) return;
    try {
      const updated = await apiFetch(`${WS_API}/${node.id}/share`, {
        method: "POST",
        body: JSON.stringify({ user_id: uid }),
      });
      Object.assign(node, updated);
      openShareDialog(node);
      dialog.close();
    } catch (e) {
      showToast("Share failed: " + e.message, "error");
    }
  });

  dialog.showModal();
}

// ── Move dialog ───────────────────────────────────────────────────────────────

async function openMoveDialog(node) {
  const dest = prompt("Enter the new parent folder path (empty = root):");
  if (dest === null) return;
  try {
    await apiFetch(`${WS_API}/${node.id}/move`, {
      method: "POST",
      body: JSON.stringify({
        new_parent_id: null,
        new_parent_path: dest.trim() || null,
      }),
    });
    showToast("Moved successfully", "success");
    loadTree(null, treeEl);
  } catch (e) {
    showToast("Move failed: " + e.message, "error");
  }
}

// ── Delete ────────────────────────────────────────────────────────────────────

async function confirmDelete(node) {
  if (!confirm(`Delete "${node.name}"? This cannot be undone.`)) return;
  try {
    await apiFetch(`${WS_API}/${node.id}`, { method: "DELETE" });
    showToast(`"${node.name}" deleted`, "success");
    if (activeNodeId === node.id) {
      activeNodeId = null;
      const url = new URL(window.location.href);
      url.searchParams.delete("ws");
      history.replaceState({}, "", url.toString());
      document.dispatchEvent(new CustomEvent("ws:deselect"));
    }
    loadTree(null, treeEl);
  } catch (e) {
    showToast("Delete failed: " + e.message, "error");
  }
}

// ── Keyboard navigation ───────────────────────────────────────────────────────

function onNodeKey(e, node) {
  switch (e.key) {
    case "Enter":
      if (node.kind === "conversation") {
        const el = treeEl.querySelector(`[data-id="${node.id}"]`);
        if (el) selectConversation(node, el);
      } else {
        e.currentTarget.closest("details")?.toggleAttribute("open");
      }
      break;
    case "F2":
      e.preventDefault();
      // TODO: inline rename — future ADR
      break;
    case "Delete":
      e.preventDefault();
      confirmDelete(node);
      break;
    case "ArrowRight": {
      const details = e.currentTarget.closest("details");
      if (details && !details.open) details.open = true;
      break;
    }
    case "ArrowLeft": {
      const details = e.currentTarget.closest("details");
      if (details && details.open) details.open = false;
      break;
    }
  }
}

function onGlobalKey(e) {
  if ((e.ctrlKey || e.metaKey) && e.key === "n") {
    e.preventDefault();
    openNewDialog(null);
  }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

// Returns the nearest enclosing folder node for the currently-selected item,
// so the "+" button creates inside the active folder rather than always at root.
// Derive the folder to create inside from the currently-selected tree item.
// Uses aria-current="page" as the source of truth rather than the module-level
// activeNodeId variable, which may not be set yet if the button is clicked
// before the first selectConversation call.
function activeParentFolder() {
  const selectedEl = treeEl?.querySelector('[aria-current="page"]');
  if (!selectedEl) return null;
  const folderDetails = selectedEl.closest("details.ws-folder");
  if (!folderDetails) return null;
  return {
    id: folderDetails.dataset.id,
    name: folderDetails.querySelector(".ws-name")?.textContent?.trim() ?? "folder",
    kind: "folder",
  };
}

function refreshParent(parentNode) {
  if (!parentNode) {
    loadTree(null, treeEl);
    return;
  }
  const details = treeEl.querySelector(
    `details.ws-folder[data-id="${parentNode.id}"]`
  );
  const childContainer = details?.querySelector(".ws-children");
  if (childContainer) loadTree(parentNode.id, childContainer);
  else loadTree(null, treeEl);
}

async function apiFetch(url, opts = {}) {
  const res = await fetch(url, {
    headers: { "Content-Type": "application/json", ...opts.headers },
    ...opts,
  });
  if (res.status === 204) return null;
  const data = await res.json();
  if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
  return data;
}

function showToast(msg, type = "info") {
  let output = document.getElementById("ws-toast");
  if (!output) {
    output = document.createElement("output");
    output.id = "ws-toast";
    output.setAttribute("role", "status");
    output.setAttribute("aria-live", "polite");
    output.className = "ws-toast";
    document.body.appendChild(output);
  }
  output.className = `ws-toast ws-toast--${type}`;
  output.textContent = msg;
  clearTimeout(output._t);
  output._t = setTimeout(() => (output.textContent = ""), 4000);
}

function escHtml(str) {
  return String(str)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

// ── Public API (consumed by app.js) ──────────────────────────────────────────

export { activeNodeId, showToast };
export function getActiveNodeId() {
  return activeNodeId;
}

// Live-save hook: call from app.js editor blur
export async function saveContent(nodeId, content) {
  try {
    await apiFetch(`${WS_API}/${nodeId}/content`, {
      method: "PATCH",
      body: JSON.stringify({ content }),
    });
  } catch (e) {
    showToast("Auto-save failed: " + e.message, "error");
  }
}
