import type { AppLocale, AppTheme } from "../store/uiStore";

export function applyUiPreferences({ locale, theme }: { locale: AppLocale; theme: AppTheme }) {
  const root = document.documentElement;
  root.lang = locale;
  root.dataset.theme = theme;
  root.classList.toggle("dark", theme === "dark");
  root.style.colorScheme = theme;
}
