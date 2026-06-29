<script lang="ts">
  // ThemeToggle — flips light↔dark, writes `data-theme` on <html>, persists the
  // choice to localStorage, and initializes from localStorage ?? prefers-color-scheme.
  // A pure UI preference: no backend round-trip.

  type Theme = "light" | "dark";
  const STORAGE_KEY = "kith-theme";

  function readStored(): Theme | null {
    try {
      const v = localStorage.getItem(STORAGE_KEY);
      return v === "light" || v === "dark" ? v : null;
    } catch {
      return null;
    }
  }

  function systemPrefersDark(): boolean {
    return window.matchMedia("(prefers-color-scheme: dark)").matches;
  }

  let theme = $state<Theme>(
    readStored() ?? (systemPrefersDark() ? "dark" : "light"),
  );

  // Apply on mount and on every change; persist best-effort.
  $effect(() => {
    document.documentElement.dataset.theme = theme;
    try {
      localStorage.setItem(STORAGE_KEY, theme);
    } catch {
      /* storage unavailable — the in-memory choice still holds this session */
    }
  });

  function toggle(): void {
    theme = theme === "dark" ? "light" : "dark";
  }

  const label = $derived(
    theme === "dark" ? "Switch to light theme" : "Switch to dark theme",
  );
</script>

<button
  class="toggle"
  type="button"
  onclick={toggle}
  aria-label={label}
  title={label}
>
  {#if theme === "dark"}
    <svg
      viewBox="0 0 24 24"
      width="18"
      height="18"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      aria-hidden="true"
    >
      <circle cx="12" cy="12" r="4" />
      <path
        d="M12 2v2M12 20v2M2 12h2M20 12h2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"
      />
    </svg>
  {:else}
    <svg
      viewBox="0 0 24 24"
      width="18"
      height="18"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
      aria-hidden="true"
    >
      <path d="M21 12.8A9 9 0 1 1 11.2 3 7 7 0 0 0 21 12.8z" />
    </svg>
  {/if}
</button>

<style>
  .toggle {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 2.25rem;
    height: 2.25rem;
    padding: 0;
    background: transparent;
    border: 1px solid var(--color-hairline);
    border-radius: var(--radius-md);
    color: var(--color-ink-soft);
    transition: color var(--motion-fast), border-color var(--motion-fast);
  }

  .toggle:hover {
    color: var(--color-ink);
    border-color: var(--color-accent);
  }
</style>
