-- DB-backed, versioned prompt capabilities.
-- Each row is an immutable prompt version; the latest version is used at runtime.
-- Prompts are loaded by DynamicPromptCapability and cached with moka.

CREATE TABLE IF NOT EXISTS dynamic_prompts (
    capability_name TEXT NOT NULL,
    version         INT  NOT NULL DEFAULT 1,
    system_prompt   TEXT,
    user_template   TEXT NOT NULL,
    few_shot        JSONB NOT NULL DEFAULT '[]',
    output_schema   JSONB,
    model           TEXT NOT NULL,
    max_tokens      INT  NOT NULL DEFAULT 1024,
    vision          BOOL NOT NULL DEFAULT false,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (capability_name, version)
);

CREATE INDEX IF NOT EXISTS dyn_prompts_latest_idx
    ON dynamic_prompts (capability_name, version DESC);
