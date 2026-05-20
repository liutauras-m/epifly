**Functional Requirement**

**ID:** FR-DOC-001  
**Title:** Multi-Document Grounded Question Answering with Automatic Context + Citations  
**Priority:** High  
**Version:** v1 (after confirmed pipeline implementation)  
**Related Components:** `ContentIngestor`, `ContextInjectionHook`, `SemanticCapabilityRouter`, `ArtifactBridge`, `QdrantVectorStore`, `BuiltinFactory`

### 1. Description

A user can upload multiple documents to their workspace. After ingestion, they can ask a specific question that requires information spread across those documents. The agent must automatically ground its answer in the uploaded content and return clear, traceable citations without the user manually calling retrieval tools.

### 2. User Scenario (Ready-to-use Prompt)

**User Action:**
1. User uploads the following files into a workspace folder (via Web or Browser Shell):
   - `Q3_2026_Financial_Report.md`
   - `EMEA_Sales_Deep_Dive.md`
   - `Q3_Leadership_Meeting_Notes.md`
   - `Revenue_Waterfall_Chart.md`

2. User then sends this message in the chat:

> "Looking at my Q3 2026 documents, what was the primary reason for the EMEA revenue shortfall? Please quote the exact section or heading from the documents that supports your answer and include the file name."

### 3. Expected System Behavior (After Implementation)

| Step | What Happens | Canonical Component |
|------|--------------|---------------------|
| 1 | Files are uploaded → `ContentIngestor` is triggered (via RustFS event or job) | `ContentIngestor` + `JobExecutor` |
| 2 | Documents are chunked (heading-aware), embedded, and stored in `content_embeddings` | `QdrantVectorStore` |
| 3 | Markdown sidecars are created/published for each file | `SidecarSyncEngine` + `ArtifactBridge` |
| 4 | User sends the question | — |
| 5 | On every `Agent` turn, `ContextInjectionHook` (`PromptHook`) automatically retrieves the most relevant chunks across all uploaded documents and prepends them to the prompt | `ContextInjectionHook` |
| 6 | `SemanticCapabilityRouter` decides whether `workspace.retrieve_context` should also be offered to the LLM | `SemanticCapabilityRouter` |
| 7 | Agent answers with clear citations including **file name + heading path** | `RetrievedChunk` structure |
| 8 | Answer is streamed back with proper formatting and citations | Existing streaming + `AgentChatStream` |

### 4. Acceptance Criteria (Testable)

- After uploading the 4 files, the user can ask the question above **without** manually triggering any retrieval capability.
- The agent’s final answer **must** contain at least one citation in this format:
  > According to **Q3_2026_Financial_Report.md → "3.2 EMEA Performance"**:
  > “The main driver of the €2.4M shortfall was delayed enterprise deal closures in Germany and France…”

- The agent correctly identifies the **primary reason** from the documents (not hallucinated).
- If the automatic context is insufficient, the agent may call `workspace.retrieve_context` (via `SemanticCapabilityRouter`) to fetch more targeted chunks.
- Citations must be accurate (correct file + heading).
- Re-uploading the same files results in `IngestStatus::Skipped` (idempotency).
- The feature respects `PlanLimits` (maximum context tokens injected).

### 5. Non-Functional Requirements

- **Performance**: Ingestion of the 4 documents should complete in < 30 seconds (background job).
- **Transparency**: User sees that context came from their uploaded files (via citations).
- **Maintainability**: All logic lives in `ContentIngestor`, `ContextInjectionHook`, and small `CapabilityProvider`s — no changes to core `Agent` runtime.
- **Extensibility**: Same flow works when PDF/Office extractors are added later.

### 6. Example Expected Output (Agent Response)

> **Answer:**  
> The primary reason for the EMEA revenue shortfall in Q3 2026 was **delayed enterprise deal closures**, particularly in Germany and France.  
>
> **Supporting evidence:**  
> - **Q3_2026_Financial_Report.md → Section "3.2 EMEA Performance"**:  
>   “The €2.4M gap versus plan was almost entirely driven by three delayed enterprise renewals in DACH and France…”  
>
> - **EMEA_Sales_Deep_Dive.md → "Key Deal Pipeline Risks"**:  
>   Lists the same three deals as “at risk due to procurement delays”.

---

Would you like me to also create:

- A **technical implementation note** version of this requirement (with exact method calls and module responsibilities)?
- A **test prompt + expected SSE events** version (for E2E testing)?
- Or variations for different scenarios (e.g. “compare these two documents”, “generate a summary table”, “export the answer as PDF”)?

Just say the word and I’ll generate it in the same clean format.