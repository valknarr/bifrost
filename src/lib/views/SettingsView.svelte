<script lang="ts">
  import { onMount } from "svelte";
  import { vaultStore } from "../stores/vault.svelte";
  import { chromiumStore } from "../stores/chromium.svelte";
  import { configStore } from "../stores/config.svelte";
  import { statusStore } from "../stores/status.svelte";
  import { sandboxieStore } from "../stores/sandboxie.svelte";
  import { externalLinks, openExternal } from "../external";
  import Button from "../components/Button.svelte";
  import VersionLink from "../components/VersionLink.svelte";
  import UninstallButton from "../components/UninstallButton.svelte";
  import CompanionSitesSection from "../components/CompanionSitesSection.svelte";
  import type { SandboxieVariant } from "../types";
  import { ZOOM_PRESETS, presetFor, type ZoomPreset } from "../zoom";
  import { vTag } from "../version";

  let loading = $state(true);

  onMount(async () => {
    try {
      await Promise.all([
        statusStore.refresh(),
        configStore.refresh(),
        vaultStore.refresh(),
        chromiumStore.refresh(),
        sandboxieStore.refresh(),
      ]);
    } finally {
      loading = false;
    }
  });

  const status = $derived(statusStore.status);
  const sandboxie = $derived(sandboxieStore.status);
  const sandboxieBusy = $derived(sandboxieStore.busy);
  const sandboxieError = $derived(sandboxieStore.error);

  // Which variant the user wants to install when nothing's on disk.
  // Plus is the recommended default; the "Use Classic instead" link
  // below the install button toggles this. Persisted only for the
  // lifetime of the Settings view — there's no point storing it
  // server-side, the user picks at install time.
  let installTarget = $state<SandboxieVariant>("plus");

  // The variant currently installed (Plus or Classic). Falls back to
  // the install target when nothing's on disk so the labels stay
  // consistent through the install flow.
  const activeVariant = $derived<SandboxieVariant>(
    sandboxie?.installedVariant ?? installTarget,
  );
  const activeLatestVersion = $derived(
    activeVariant === "plus"
      ? sandboxie?.latestPlusVersion
      : sandboxie?.latestClassicVersion,
  );
  const variantLabel = $derived(
    activeVariant === "plus" ? "Plus" : "Classic",
  );
  const installTargetLatest = $derived(
    installTarget === "plus"
      ? sandboxie?.latestPlusVersion
      : sandboxie?.latestClassicVersion,
  );
  const installTargetLabel = $derived(
    installTarget === "plus" ? "Plus" : "Classic",
  );
  const otherVariant = $derived<SandboxieVariant>(
    installTarget === "plus" ? "classic" : "plus",
  );
  const otherVariantLabel = $derived(
    otherVariant === "plus" ? "Plus" : "Classic",
  );

  // Active zoom preset, derived from the persisted config so the
  // picker visually marks the right button after a launch / reload.
  // If a user ever lands on a custom value (e.g. via a future
  // keyboard shortcut), all three preset buttons stay un-pressed.
  const activeZoom = $derived(configStore.config?.uiZoom ?? 1.0);
  const activePreset = $derived<ZoomPreset | "custom">(presetFor(activeZoom));

  // Reactive aliases so the markup stays readable.
  const config = $derived(configStore.config);
  const configError = $derived(configStore.error); // surfaced by the zoom picker
  const vault = $derived(vaultStore.status);
  const vaultBusy = $derived(vaultStore.busy);
  const vaultError = $derived(vaultStore.error);
  const chromium = $derived(chromiumStore.status);
  const chromiumBusy = $derived(chromiumStore.busy);
  const chromiumError = $derived(chromiumStore.error);

  function formatBytes(bytes: number | null | undefined): string {
    if (!bytes) return "?";
    const mb = bytes / 1_048_576;
    return mb >= 1 ? `${mb.toFixed(0)} MB` : `${(bytes / 1024).toFixed(0)} KB`;
  }

  function tone(ok: boolean): string {
    return ok ? "text-[var(--color-ok)]" : "text-[var(--color-danger)]";
  }
  function tag(ok: boolean): string {
    return ok ? "● Detected" : "✕ Missing";
  }
</script>

<section
  class="relative mx-auto flex w-full max-w-6xl flex-col gap-6 border border-[var(--color-border)] bg-[var(--color-surface)]/40"
>
  <!-- Panel header. 2 px accent-tinted bottom border echoes
       CradleOS's HUD-style section dividers; inner section labels
       below the header keep the lighter 1 px rule.
       `min-h-[40px]` matches the rule in PilotsView so the header
       doesn't grow / shrink by a pixel or two when tabbing between
       the views — see the comment over there for the arithmetic. -->
  <header class="border-b-2 border-[var(--color-accent)]/40 px-6 pt-5 pb-4">
    <div class="flex min-h-[40px] items-center gap-4">
      <h1 class="title-bracket text-lg tracking-[0.05em] text-[var(--color-text)]">
        System Configuration
      </h1>
    </div>
  </header>

  <div class="flex flex-col gap-8 px-6 pb-8">
    {#if loading}
      <div class="text-[calc(11px*var(--text-scale,1))] text-[var(--color-text-muted)] tracking-[0.2em] uppercase">
        Reading state…
      </div>
    {:else}
      <!-- Detection -->
      <div class="flex flex-col gap-3">
        <div class="flex items-baseline justify-between">
          <h2 class="section-label text-[var(--color-accent)]">Detection</h2>
          <span class="text-[calc(10px*var(--text-scale,1))] text-[var(--color-text-dim)] tracking-[0.2em] uppercase">
            Live probe of the host
          </span>
        </div>
        <div
          class="flex flex-col gap-3 border border-[var(--color-border)] bg-[var(--color-bg)] px-5 py-4"
        >
          <!-- Sandboxie row (Plus or Classic — both variants share the
               kernel driver + CLI surface, so one row covers both). -->
          <div class="flex flex-col gap-2">
            <div class="field">
              <span class="label">Sandboxie</span>
              <span class="leader"></span>
              <span class="value {tone(status?.sandboxieInstalled ?? false)}">
                {tag(status?.sandboxieInstalled ?? false)}
                {#if sandboxie?.installedVariant}
                  <span class="ml-1 text-[var(--color-text-dim)]">
                    · {variantLabel}
                  </span>
                {/if}
                {#if sandboxie?.installedVersion}
                  <span class="ml-1 text-[var(--color-text-dim)]">
                    · <VersionLink
                      version={sandboxie.installedVersion}
                      url={externalLinks.sandboxie}
                    />
                    {#if sandboxie.updateAvailable && activeLatestVersion}
                      <span class="text-[var(--color-warn)]">
                        · update available · <VersionLink
                          version={activeLatestVersion}
                          url={externalLinks.sandboxie}
                          class="text-[var(--color-warn)]"
                        />
                      </span>
                    {:else if sandboxie.detected}
                      · up to date
                    {/if}
                  </span>
                {:else if status?.sandboxieInstalled && activeLatestVersion}
                  <span class="ml-1 text-[var(--color-text-dim)]">
                    · external · latest <VersionLink
                      version={activeLatestVersion}
                      url={externalLinks.sandboxie}
                    />
                  </span>
                {:else if installTargetLatest}
                  <span class="ml-1 text-[var(--color-text-dim)]">
                    · latest {installTargetLabel} <VersionLink
                      version={installTargetLatest}
                      url={externalLinks.sandboxie}
                    />
                  </span>
                {/if}
              </span>
            </div>

            {#if sandboxie?.latestError && !sandboxie?.latestPlusVersion && !sandboxie?.latestClassicVersion}
              <p
                class="border-l border-[var(--color-warn)]/50 pl-3 text-[calc(10px*var(--text-scale,1))] leading-snug text-[var(--color-warn)]"
              >
                {sandboxie.latestError}
              </p>
            {/if}

            <!-- Action row — single Install when missing (with a
                 variant-toggle link below), otherwise the cluster sits
                 flush-right under the status line: Check for updates +
                 trash. -->
            {#if !(status?.sandboxieInstalled ?? false)}
              <div class="flex flex-col gap-1 pt-1">
                <div class="flex justify-end">
                  <Button
                    variant="primary"
                    size="sm"
                    disabled={sandboxieBusy}
                    onclick={() => sandboxieStore.install(installTarget)}
                    title={`Download + run the Sandboxie ${installTargetLabel} installer silently (one UAC prompt)`}
                  >
                    {#if sandboxieBusy}
                      Installing…
                    {:else if installTargetLatest}
                      ▼ Install {installTargetLabel} {vTag(installTargetLatest)}
                    {:else}
                      ▼ Install {installTargetLabel}
                    {/if}
                  </Button>
                </div>
                <div class="flex justify-end">
                  <button
                    type="button"
                    class="cursor-pointer text-[calc(10px*var(--text-scale,1))] tracking-[0.05em] text-[var(--color-text-muted)] underline-offset-2 hover:text-[var(--color-focus)] hover:underline disabled:opacity-30"
                    disabled={sandboxieBusy}
                    onclick={() => (installTarget = otherVariant)}
                    title="Switch which Sandboxie variant the install button targets"
                  >
                    Use {otherVariantLabel} instead
                  </button>
                </div>
                {#if installTarget === "classic"}
                  <!-- Classic's kernel driver predates Windows 10/11
                       console-subsystem updates; SbieDll proxies a
                       few IPC calls that the older driver returns
                       NOT_IMPLEMENTED for. Logged as SBIE2205 lines
                       but harmless — EVE Frontier launches and runs
                       regardless. Surfacing this so users who pick
                       Classic aren't surprised by their log. -->
                  <p
                    class="border-l border-[var(--color-warn)]/40 pl-3 text-[calc(10px*var(--text-scale,1))] leading-snug text-[var(--color-warn)]"
                  >
                    ⓘ Classic logs benign
                    <span class="mono">SBIE2205 ConsoleInit</span>
                    warnings on Windows 10 21H2 and later — its kernel
                    driver doesn't implement the IPC paths Plus added
                    for the newer console subsystem. Non-fatal: EVE
                    Frontier still launches and plays. Plus ships the
                    updated driver if you'd prefer a quieter log.
                  </p>
                {/if}
              </div>
            {:else if sandboxie?.updateAvailable && sandboxie.installedVersion}
              <div class="flex items-center justify-end gap-2 pt-1">
                <Button
                  variant="primary"
                  size="sm"
                  disabled={sandboxieBusy}
                  onclick={() => sandboxieStore.install(activeVariant)}
                >
                  {sandboxieBusy
                    ? "Updating…"
                    : `Update to v${activeLatestVersion}`}
                </Button>
                <UninstallButton
                  component={`Sandboxie ${variantLabel}`}
                  warning="Sandboxie ships a kernel driver — uninstalling will trigger a Windows UAC prompt. Close the EVE Frontier launcher and any running sandboxes first, or the uninstall will fail half-way. Pilots using Sandboxie sandboxes will stop working until you reinstall."
                  busy={sandboxieBusy}
                  onConfirm={() => sandboxieStore.uninstall()}
                />
              </div>
            {:else}
              <div class="flex items-center justify-end gap-2 pt-1">
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={sandboxieBusy}
                  onclick={() => sandboxieStore.refresh(true)}
                >
                  Check for updates
                </Button>
                <UninstallButton
                  component={`Sandboxie ${variantLabel}`}
                  warning="Sandboxie ships a kernel driver — uninstalling will trigger a Windows UAC prompt. Close the EVE Frontier launcher and any running sandboxes first, or the uninstall will fail half-way. Pilots using Sandboxie sandboxes will stop working until you reinstall."
                  busy={sandboxieBusy}
                  onConfirm={() => sandboxieStore.uninstall()}
                />
              </div>
            {/if}

            {#if sandboxieError}
              <div
                class="border border-[var(--color-danger)]/40 bg-[var(--color-danger)]/10 px-3 py-2 text-[calc(10px*var(--text-scale,1))] text-[var(--color-danger)]"
              >
                {sandboxieError}
              </div>
            {/if}

            {#if !(status?.sandboxieInstalled ?? false)}
              <p
                class="text-[calc(10px*var(--text-scale,1))] leading-snug text-[var(--color-text-muted)]"
              >
                Bifrost installs Sandboxie-Plus by default. Sandboxie
                Classic is also supported — same kernel driver, lighter
                UI. Only one variant can be installed at a time; to
                switch, uninstall first then install the other.
                Windows requires one UAC prompt to grant the installer
                kernel-driver permissions.
              </p>
            {/if}
          </div>

          <div class="h-px bg-[var(--color-border)]"></div>

          <!-- EVE Frontier client row -->
          <div class="flex flex-col gap-1">
            <div class="field">
              <span class="label">EVE Frontier client</span>
              <span class="leader"></span>
              <span class="value {tone(status?.frontierFound ?? false)}">
                {tag(status?.frontierFound ?? false)}
              </span>
            </div>
            {#if !(status?.frontierFound ?? false)}
              <div
                class="flex items-center justify-between gap-3 border-l border-[var(--color-danger)]/40 pl-3"
              >
                <span class="text-[calc(10px*var(--text-scale,1))] text-[var(--color-text-muted)] leading-snug">
                  The official game launcher. Bifrost will launch it inside
                  each pilot's sandbox.
                </span>
                <button
                  class="mono shrink-0 cursor-pointer border border-[var(--color-border-hi)] px-2.5 py-1 text-[calc(10px*var(--text-scale,1))] tracking-[0.18em] text-[var(--color-text-muted)] uppercase transition-colors hover:border-[var(--color-focus)] hover:text-[var(--color-focus)]"
                  onclick={() => openExternal(externalLinks.eveFrontier)}
                  title={externalLinks.eveFrontier}
                >
                  Download ↗
                </button>
              </div>
            {/if}
          </div>
        </div>
      </div>

      <!-- EVE Vault integration — no explicit on/off toggle. Bifrost
           considers the integration "active" whenever both Brave and
           the EVE Vault extension are installed; the two install rows
           below are the only controls needed. -->
      <div class="flex flex-col gap-3">
        <div class="flex items-baseline justify-between">
          <h2 class="section-label">EVE Vault Integration</h2>
          <span class="text-[calc(10px*var(--text-scale,1))] text-[var(--color-text-dim)] tracking-[0.2em] uppercase">
            Per-pilot browser + wallet
          </span>
        </div>

        <div class="flex flex-col gap-4 border border-[var(--color-border)] bg-[var(--color-bg)] px-5 py-4">
          <!-- Portable Chromium -->
          <div class="flex flex-col gap-2">
            <div class="flex items-baseline justify-between">
              <span
                class="text-[calc(10px*var(--text-scale,1))] tracking-[0.25em] text-[var(--color-accent)] uppercase"
              >
                Portable browser
              </span>
              <span
                class="text-[calc(10px*var(--text-scale,1))] text-[var(--color-text-dim)] tracking-[0.15em] uppercase"
              >
                Brave · {formatBytes(chromium?.latestSizeBytes)}
              </span>
            </div>
            <!-- Single status line, matches Sandboxie row -->
            <div class="field">
              <span class="label">Brave</span>
              <span class="leader"></span>
              <span class="value {tone(!!chromium?.installedVersion)}">
                {chromium?.installedVersion ? "● Installed" : "○ Not installed"}
                {#if chromium?.installedVersion}
                  <span class="ml-1 text-[var(--color-text-dim)]">
                    · <VersionLink
                      version={chromium.installedVersion}
                      url={externalLinks.brave}
                    />
                    {#if chromium.updateAvailable && chromium.latestVersion}
                      <span class="text-[var(--color-warn)]">
                        · update available · <VersionLink
                          version={chromium.latestVersion}
                          url={externalLinks.brave}
                          class="text-[var(--color-warn)]"
                        />
                      </span>
                    {:else if chromium.latestVersion}
                      · up to date
                    {:else}
                      <!-- latest_version is null when the upstream
                           release exists but the asset isn't uploaded
                           yet, OR when the GitHub fetch failed. In
                           both cases "up to date" would be a lie —
                           we don't know. The error banner below tells
                           the user what happened. -->
                      · latest unknown
                    {/if}
                  </span>
                {:else if chromium?.latestVersion}
                  <span class="ml-1 text-[var(--color-text-dim)]">
                    · latest <VersionLink
                      version={chromium.latestVersion}
                      url={externalLinks.brave}
                    />
                  </span>
                {/if}
              </span>
            </div>
            {#if chromium?.latestError && !chromium?.latestVersion}
              <p
                class="border-l border-[var(--color-warn)]/50 pl-3 text-[calc(10px*var(--text-scale,1))] leading-snug text-[var(--color-warn)]"
              >
                {chromium.latestError}
              </p>
            {/if}
            {#if chromiumError}
              <div
                class="border border-[var(--color-danger)]/40 bg-[var(--color-danger)]/10 px-3 py-2 text-[calc(10px*var(--text-scale,1))] text-[var(--color-danger)]"
              >
                {chromiumError}
              </div>
            {/if}
            {#if !chromium?.installedVersion}
              <div class="flex justify-end pt-1">
                <Button
                  variant="primary"
                  size="sm"
                  disabled={chromiumBusy}
                  onclick={() => chromiumStore.install()}
                >
                  {#if chromiumBusy}
                    Downloading…
                  {:else if chromium?.latestVersion}
                    ▼ Install {vTag(chromium.latestVersion)}
                  {:else}
                    ▼ Install latest
                  {/if}
                </Button>
              </div>
            {:else if chromium?.updateAvailable}
              <div class="flex items-center justify-end gap-2 pt-1">
                <Button
                  variant="primary"
                  size="sm"
                  disabled={chromiumBusy}
                  onclick={() => chromiumStore.install()}
                >
                  {chromiumBusy
                    ? "Updating…"
                    : `Update to ${vTag(chromium.latestVersion)}`}
                </Button>
                <UninstallButton
                  component="the portable browser"
                  warning="Per-pilot browser profiles (wallet sessions, extensions) are kept under each pilot — only the Brave binaries are removed."
                  busy={chromiumBusy}
                  onConfirm={() => chromiumStore.uninstall()}
                />
              </div>
            {:else}
              <div class="flex items-center justify-end gap-2 pt-1">
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={chromiumBusy}
                  onclick={() => chromiumStore.refresh(true)}
                >
                  Check for updates
                </Button>
                <UninstallButton
                  component="the portable browser"
                  warning="Per-pilot browser profiles (wallet sessions, extensions) are kept under each pilot — only the Brave binaries are removed."
                  busy={chromiumBusy}
                  onConfirm={() => chromiumStore.uninstall()}
                />
              </div>
            {/if}
          </div>

          <div class="h-px bg-[var(--color-border)]"></div>

          <!-- EVE Vault extension -->
          <div class="flex flex-col gap-2">
            <div class="flex items-baseline justify-between">
              <span
                class="text-[calc(10px*var(--text-scale,1))] tracking-[0.25em] text-[var(--color-accent)] uppercase"
              >
                EVE Vault extension
              </span>
              <span
                class="text-[calc(10px*var(--text-scale,1))] text-[var(--color-text-dim)] tracking-[0.15em] uppercase"
              >
                Verified · SHA-256
              </span>
            </div>
            <!-- Single status line, matches Sandboxie + Brave rows -->
            <div class="field">
              <span class="label">EVE Vault</span>
              <span class="leader"></span>
              <span class="value {tone(!!vault?.installedVersion)}">
                {vault?.installedVersion ? "● Installed" : "○ Not installed"}
                {#if vault?.installedVersion}
                  <span class="ml-1 text-[var(--color-text-dim)]">
                    · <VersionLink
                      version={vault.installedVersion}
                      url={externalLinks.eveVault}
                    />
                    {#if vault.updateAvailable && vault.latestVersion}
                      <span class="text-[var(--color-warn)]">
                        · update available · <VersionLink
                          version={vault.latestVersion}
                          url={externalLinks.eveVault}
                          class="text-[var(--color-warn)]"
                        />
                      </span>
                    {:else if vault.latestVersion}
                      · up to date
                    {:else}
                      · latest unknown
                    {/if}
                  </span>
                {:else if vault?.latestVersion}
                  <span class="ml-1 text-[var(--color-text-dim)]">
                    · latest <VersionLink
                      version={vault.latestVersion}
                      url={externalLinks.eveVault}
                    />
                  </span>
                {/if}
              </span>
            </div>
            {#if vault?.latestError && !vault?.latestVersion}
              <p
                class="border-l border-[var(--color-warn)]/50 pl-3 text-[calc(10px*var(--text-scale,1))] leading-snug text-[var(--color-warn)]"
              >
                {vault.latestError}
              </p>
            {/if}
            {#if vaultError}
              <div
                class="border border-[var(--color-danger)]/40 bg-[var(--color-danger)]/10 px-3 py-2 text-[calc(10px*var(--text-scale,1))] text-[var(--color-danger)]"
              >
                {vaultError}
              </div>
            {/if}
            {#if !vault?.installedVersion}
              <div class="flex justify-end pt-1">
                <Button
                  variant="primary"
                  size="sm"
                  disabled={vaultBusy}
                  onclick={() => vaultStore.install()}
                >
                  {#if vaultBusy}
                    Installing…
                  {:else if vault?.latestVersion}
                    ▼ Install {vTag(vault.latestVersion)}
                  {:else}
                    ▼ Install latest
                  {/if}
                </Button>
              </div>
            {:else if vault?.updateAvailable}
              <div class="flex items-center justify-end gap-2 pt-1">
                <Button
                  variant="primary"
                  size="sm"
                  disabled={vaultBusy}
                  onclick={() => vaultStore.install()}
                >
                  {vaultBusy
                    ? "Updating…"
                    : `Update to ${vTag(vault.latestVersion)}`}
                </Button>
                <UninstallButton
                  component="the EVE Vault extension"
                  warning="Existing browser windows keep their loaded extension until they reload."
                  busy={vaultBusy}
                  onConfirm={() => vaultStore.uninstall()}
                />
              </div>
            {:else}
              <div class="flex items-center justify-end gap-2 pt-1">
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={vaultBusy}
                  onclick={() => vaultStore.refresh(true)}
                >
                  Check for updates
                </Button>
                <UninstallButton
                  component="the EVE Vault extension"
                  warning="Existing browser windows keep their loaded extension until they reload."
                  busy={vaultBusy}
                  onConfirm={() => vaultStore.uninstall()}
                />
              </div>
            {/if}
          </div>

          <p class="text-[calc(10px*var(--text-scale,1))] tracking-[0.04em] text-[var(--color-text-dim)] leading-relaxed">
            Bifrost downloads the official EVE Vault Chromium extension from
            <span class="mono">github.com/evefrontier/evevault</span>
            (verified by SHA-256) and stores it under Bifrost's app-data.
            Each pilot's "Apps" buttons open ecosystem sites in a
            standalone Chromium window with EVE Vault preloaded as that
            pilot's identity.
          </p>
          <p class="text-[calc(10px*var(--text-scale,1))] tracking-[0.04em] text-[var(--color-text-dim)] leading-relaxed">
            <span class="text-[var(--color-warn)]">One-time setup per pilot:</span>
            the first browser launch shows a Chromium window with EVE
            Vault pinned to the toolbar. Click the wallet icon → sign in
            via FusionAuth → derives the Sui address via zkLogin.
            Bifrost can't automate this step (it requires your credentials)
            but only happens once per pilot — the profile remembers from
            then on.
          </p>
        </div>
      </div>

      <!-- Display — UI text-scale presets. Writes the
           `--text-scale` CSS variable on `:root` so rem-based type
           sizes scale together while pixel-based chrome (button
           hit-targets, padding, grid gaps) stays fixed — the
           desktop-app shape, not the browser-zoom shape. Choice is
           persisted in BifrostConfig.uiZoom and re-applied on next
           launch by App.svelte. -->
      <div class="flex flex-col gap-3">
        <div class="flex items-baseline justify-between">
          <h2 class="section-label">Display</h2>
          <span
            class="text-[calc(10px*var(--text-scale,1))] text-[var(--color-text-dim)] tracking-[0.2em] uppercase"
          >
            UI scale · persists across launches
          </span>
        </div>

        <div
          class="flex flex-col gap-3 border border-[var(--color-border)] bg-[var(--color-bg)] px-5 py-4"
        >
          <div class="field">
            <span class="label">UI scale</span>
            <span class="leader"></span>
            <span class="value text-[var(--color-text-muted)]">
              {#if activePreset === "custom"}
                Custom · {(activeZoom * 100).toFixed(0)}%
              {:else}
                {ZOOM_PRESETS[activePreset].label} · {(
                  ZOOM_PRESETS[activePreset].value * 100
                ).toFixed(0)}%
              {/if}
            </span>
          </div>

          <!-- Picker. Each button is `primary` when active, `ghost`
               otherwise — so the selected preset glows accent-orange
               and the other two read as available alternatives. -->
          <div class="flex flex-wrap items-center gap-2">
            {#each ["compact", "default", "comfortable"] as key (key)}
              {@const preset = ZOOM_PRESETS[key as ZoomPreset]}
              <Button
                variant={activePreset === key ? "primary" : "ghost"}
                size="sm"
                onclick={() => configStore.setUiZoom(preset.value)}
                title={preset.description}
              >
                {preset.label}
              </Button>
            {/each}
          </div>

          {#if configError}
            <p
              class="border-l border-[var(--color-danger)]/50 pl-3 text-[calc(10px*var(--text-scale,1))] leading-snug text-[var(--color-danger)]"
            >
              {configError}
            </p>
          {/if}

          <p
            class="text-[calc(10px*var(--text-scale,1))] leading-snug text-[var(--color-text-muted)]"
          >
            Scales text only — button hit-targets, padding, and grid
            gaps stay at their natural pixel sizes regardless of
            preset. Compact reads as denser type in the same chrome;
            Comfortable as bigger type in the same chrome. The choice
            survives restarts.
          </p>

          <!-- Roster layout. Lets the user lock the pilot grid to a
               fixed column count instead of the responsive default.
               Stored as `rosterColumns` in BifrostConfig (0 = auto,
               2 = two cards per row, 3 = three). PilotsView reacts
               to the value via configStore — no extra view-glue. -->
          <div class="mt-2 flex flex-col gap-2 border-t border-[var(--color-border)] pt-3">
            <div class="field">
              <span class="label">Roster layout</span>
              <span class="leader"></span>
              <span class="value text-[var(--color-text-muted)]">
                {#if (config?.rosterColumns ?? 0) === 0}
                  Auto · fills the window
                {:else}
                  {config?.rosterColumns} pilots per row
                {/if}
              </span>
            </div>
            <div class="flex flex-wrap items-center gap-2">
              {#each [
                { value: 0, label: "Auto", title: "Responsive — as many pilots per row as the window can hold." },
                { value: 2, label: "2 pilots", title: "Lock the grid to two pilots per row. Best for narrower windows." },
                { value: 3, label: "3 pilots", title: "Lock the grid to three pilots per row. Best for wider windows." },
              ] as option (option.value)}
                <Button
                  variant={(config?.rosterColumns ?? 0) === option.value ? "primary" : "ghost"}
                  size="sm"
                  onclick={() => configStore.setRosterColumns(option.value)}
                  title={option.title}
                >
                  {option.label}
                </Button>
              {/each}
            </div>
            <p
              class="text-[calc(10px*var(--text-scale,1))] leading-snug text-[var(--color-text-muted)]"
            >
              Picks how many pilot cards line up per row. Auto adds
              columns as you drag the window wider — 3, 4, 5, as many
              as fit at the natural card width. The fixed options (2
              or 3) snap the window to that layout and lock it,
              regardless of how you resize afterwards. The choice
              persists, so the same layout reopens next launch.
            </p>
          </div>
        </div>
      </div>

      <CompanionSitesSection />

      <!-- Paths -->
      <div class="flex flex-col gap-3">
        <div class="flex items-baseline justify-between">
          <h2 class="section-label">Paths</h2>
          <span class="text-[calc(10px*var(--text-scale,1))] text-[var(--color-text-dim)] tracking-[0.2em] uppercase">
            Edit in a later milestone
          </span>
        </div>
        <dl class="flex flex-col gap-2 border border-[var(--color-border)] bg-[var(--color-bg)] px-5 py-4">
          <div class="field">
            <span class="label">Sandboxie root</span>
            <span class="leader"></span>
            <span class="value mono text-[calc(10px*var(--text-scale,1))] text-[var(--color-text-muted)]">
              {config?.sandboxiePath ?? "—"}
            </span>
          </div>
          <div class="field">
            <span class="label">Frontier exe</span>
            <span class="leader"></span>
            <span class="value mono text-[calc(10px*var(--text-scale,1))] text-[var(--color-text-muted)]">
              {config?.frontierExe ?? "—"}
            </span>
          </div>
          <div class="field">
            <span class="label">Pilots dir</span>
            <span class="leader"></span>
            <span class="value mono text-[calc(10px*var(--text-scale,1))] text-[var(--color-text-muted)]">
              {config?.pilotsDir ?? "—"}
            </span>
          </div>
        </dl>
      </div>
    {/if}
  </div>
</section>
