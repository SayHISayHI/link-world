-- Purpose: separate provider identity from the wire protocol used by its model API.
-- Risk: medium; adds a NOT NULL column with a backward-compatible default.
-- Rollback: restore the pre-migration database; SQLite column removal is intentionally avoided.
ALTER TABLE model_provider_configs
ADD COLUMN api_family TEXT NOT NULL DEFAULT 'openai_chat_completions' CHECK (
    api_family IN (
        'openai_chat_completions',
        'openai_responses',
        'anthropic_messages',
        'google_generative_ai',
        'ollama'
    )
);
