import { useState, useEffect } from 'react';
import { getAppSetting, setAppSetting } from '../api';

export type ThemePreference = 'light' | 'dark' | 'system';

function applyTheme(preference: ThemePreference, systemDark: boolean): void {
  const resolved = preference === 'system' ? (systemDark ? 'dark' : 'light') : preference;
  if (resolved === 'dark') {
    document.documentElement.classList.add('dark');
  } else {
    document.documentElement.classList.remove('dark');
  }
}

export function useTheme() {
  const [preference, setPreference] = useState<ThemePreference>(() => {
    const cached = localStorage.getItem('theme') as ThemePreference | null;
    return cached ?? 'system';
  });

  const [systemDark, setSystemDark] = useState(
    () => window.matchMedia('(prefers-color-scheme: dark)').matches,
  );

  // On mount: load authoritative value from backend
  useEffect(() => {
    getAppSetting('theme')
      .then((value) => {
        const backendPref = (value as ThemePreference | null) ?? 'system';
        setPreference(backendPref);
        localStorage.setItem('theme', backendPref);
      })
      .catch((err) => console.error('Failed to load theme setting:', err));
  }, []);

  // Apply .dark class whenever preference or systemDark changes
  useEffect(() => {
    applyTheme(preference, systemDark);
  }, [preference, systemDark]);

  // Always listen to system preference changes so systemDark is never stale
  useEffect(() => {
    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    const handler = (e: MediaQueryListEvent) => setSystemDark(e.matches);
    mq.addEventListener('change', handler);
    return () => mq.removeEventListener('change', handler);
  }, []);

  const theme: 'light' | 'dark' =
    preference === 'system' ? (systemDark ? 'dark' : 'light') : preference;

  const setThemePreference = (pref: ThemePreference) => {
    setPreference(pref);
    localStorage.setItem('theme', pref);
    setAppSetting('theme', pref).catch((err) =>
      console.error('Failed to save theme setting:', err),
    );
  };

  return { theme, themePreference: preference, setThemePreference };
}
