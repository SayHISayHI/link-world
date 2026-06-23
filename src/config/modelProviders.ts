import type { ModelApiFamily } from "../types/api";

export interface ModelProviderPreset {
  apiFamily: ModelApiFamily;
  chatBaseUrl: string;
  chatModel: string;
}

export const MODEL_PROVIDER_PRESETS: Record<string, ModelProviderPreset> = {
  openai: {
    apiFamily: "openai_chat_completions",
    chatBaseUrl: "https://api.openai.com/v1",
    chatModel: "gpt-4.1-mini",
  },
  anthropic: {
    apiFamily: "anthropic_messages",
    chatBaseUrl: "https://api.anthropic.com/v1",
    chatModel: "claude-sonnet-4-5",
  },
  google: {
    apiFamily: "google_generative_ai",
    chatBaseUrl: "https://generativelanguage.googleapis.com/v1beta",
    chatModel: "gemini-2.5-flash",
  },
  deepseek: {
    apiFamily: "openai_chat_completions",
    chatBaseUrl: "https://api.deepseek.com",
    chatModel: "deepseek-chat",
  },
  openrouter: {
    apiFamily: "openai_chat_completions",
    chatBaseUrl: "https://openrouter.ai/api/v1",
    chatModel: "openai/gpt-4.1-mini",
  },
  groq: {
    apiFamily: "openai_chat_completions",
    chatBaseUrl: "https://api.groq.com/openai/v1",
    chatModel: "llama-3.3-70b-versatile",
  },
  xai: {
    apiFamily: "openai_chat_completions",
    chatBaseUrl: "https://api.x.ai/v1",
    chatModel: "grok-3-mini",
  },
  ollama: {
    apiFamily: "ollama",
    chatBaseUrl: "http://127.0.0.1:11434",
    chatModel: "llama3.2",
  },
};

export const MODEL_API_FAMILY_OPTIONS: Array<{ value: ModelApiFamily; label: string }> = [
  { value: "openai_chat_completions", label: "OpenAI Chat Completions" },
  { value: "openai_responses", label: "OpenAI Responses" },
  { value: "anthropic_messages", label: "Anthropic Messages" },
  { value: "google_generative_ai", label: "Google Generative AI" },
  { value: "ollama", label: "Ollama Chat" },
];
