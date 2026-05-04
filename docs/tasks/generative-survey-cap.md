**Yes – this is a textbook 2026 use-case for declarative generative UI.**  
As an *end-user* of the ConusAI agent system you don’t need to touch the core Rust crates. You just register two new `CapabilityProvider` instances (predefined templates) via the existing `/admin/capabilities` API (or the future user-facing “Templates” UI).  

The **best architecture** (validated against 2026 patterns from AG-UI, A2UI/Open-JSON-UI, and production agent platforms) is:

**Hybrid Declarative Generative UI**  
- **Predefined component catalog** (survey-form + leaderboard-table) stored in the `CapabilityRegistry`.  
- Agent emits `UIArtifact` via `GenerativeUICapability` (already in v0.3).  
- Templates are **statically bound** to dedicated capabilities (`SurveyCapability`, `LeaderboardCapability`) for consistency, validation, and security.  
- Tool bindings remain live (submit survey → calls your custom capability; refresh leaderboard → calls data provider).  
- Sharing uses the existing workspace sharing + a new public render endpoint (no auth, token-scoped).  

This follows **SRP**, reuses every v0.3 primitive (`CapabilityProvider`, `UIArtifact`, `CapabilityCard`, Rig structured output), and matches the canonical 2026 declarative pattern (structured JSON + component registry instead of open code-gen).

### 1. Updated Domain Model (zero breaking changes)

Extend `crates/common/src/ui.rs` (you can do this as a user by submitting a manifest; core team will merge in <2 AI-hours):

```rust
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub enum UIComponent {
    SurveyForm,
    LeaderboardTable,
    // ... existing ones
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UIArtifact {
    pub id: Ulid,
    pub component: UIComponent,      // now strongly typed
    pub title: String,
    pub props: serde_json::Value,    // validated against component schema
    pub tool_bindings: Vec<ToolBinding>,
    pub auto_refresh_ms: Option<u64>,
    pub share_token: Option<String>, // populated on share
}
```

### 2. Predefined Capabilities (user-registerable)

Create **two** new `CapabilityProvider` implementations (you register them once via POST `/admin/capabilities` with a manifest JSON). They live in a new user crate or via WASM if you prefer sandboxing.

```rust
// Example: crates/user-capabilities/src/survey.rs (or WASM component)
pub struct SurveyCapability {
    // can embed default questions, validation rules, etc.
}

#[async_trait]
impl CapabilityProvider for SurveyCapability {
    fn card(&self) -> CapabilityCard {
        CapabilityCard {
            name: "survey_template".into(),
            description: "Predefined survey form with live submission binding".into(),
            semantic_embedding: /* Qdrant auto-indexed */,
            ui_metadata: Some(UIComponent::SurveyForm),
            // ...
        }
    }

    async fn invoke(&self, ctx: &CapabilityContext) -> Result<CapabilityOutput> {
        // 1. Build default props from context (questions, title, etc.)
        // 2. Use Rig structured output to let the agent customise props safely
        let artifact = UIArtifact {
            component: UIComponent::SurveyForm,
            props: json!({ "questions": [...], "submit_label": "Send" }),
            tool_bindings: vec![ToolBinding {
                capability: "survey_submission".into(), // your custom handler
                action: "submit".into(),
                param_mapping: json!({ "response": "$.answers" }),
            }],
            ..Default::default()
        };
        Ok(CapabilityOutput::UiArtifact(artifact))
    }
}
```

Same pattern for `LeaderboardCapability` (component = `LeaderboardTable`, binds to a data-refresh capability).

Register once:

```bash
curl -X POST http://localhost:8080/admin/capabilities \
  -H "Authorization: Bearer <super_admin_jwt>" \
  -d @survey-manifest.json
```

The manifest contains the above struct + WASM binary (optional) or Rust crate name.

### 3. Sharing Outside the System (public & embeddable)

Extend the existing `/v1/workspaces/{id}/share` endpoint (already in v0.3 protected router) with a new flag `ui_only: true`.

**New public route** (added to `public_router` – 4 AI-hours):

```rust
// agent-gateway/src/routes/public/ui.rs
GET /v1/ui/shared/{share_token}
```

- Validates token (short-lived or permanent, stored in object_store).
- Returns either:
  - JSON `UIArtifact` (for your external Next.js/React renderer), **or**
  - Fully rendered static HTML (Askama template + Tailwind) for zero-JS embeds.
- CORS is already configured for any `WEB_ORIGIN`.

**External rendering options (2026 best practice):**
- **Your own frontend** – consume JSON exactly as the internal Askama renderer does.
- **Embed widget** – one-line `<iframe src="https://your-conusai/v1/ui/shared/abc123">` or Web Component.
- **Static export** – POST to a new `/v1/ui/export` that saves HTML/JSON to MinIO with public URL (expires or permanent).

All sharing reuses the existing `object_store` + `share_token` flow – no new storage layer.

### 4. How the Agent Decides to Use It

Your main `Agent` (via `GenerativeUICapability`) now sees both templates in the registry (semantic search via `rig-qdrant`).  
When the conversation needs a survey or leaderboard, the LLM calls the capability → gets a perfectly formed `UIArtifact` with live tool bindings.  
No custom prompt engineering required after initial registration.

### 5. Effort & Token Estimate (as end-user)

| Task                              | AI-hours | Tokens   | Who does it |
|-----------------------------------|----------|----------|-------------|
| Define & register 2 capabilities  | 6–8     | ~45k    | You (via API) |
| Add UIComponent variants + Askama | 3–4     | ~25k    | Core team (PR) |
| Public shared UI endpoint         | 4       | ~30k    | Core team   |
| **Total**                         | **13–16** | **~100k** | —           |

This is **the newest idiomatic 2026 pattern**: declarative component catalog + `CapabilityProvider`-bound templates + token-scoped public rendering. It is fully compliant with v0.3 (no architecture changes needed) and scales to any future UI (dashboards, calendars, etc.).

Want me to:
1. Generate the exact `survey-manifest.json` + `leaderboard-manifest.json` you can POST today, **or**
2. Drop the full Rust code for both `CapabilityProvider` impls + the public render route?

Just say which one (or both) and I’ll output production-ready files.