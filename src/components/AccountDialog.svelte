<script lang="ts">
  // アカウント追加画面。
  // 主要なサービスはメールアドレスから接続先を自動設定し、それ以外は手入力する。
  // 「追加」を押すと実際にサーバーへログインして確かめ、成功した場合だけ保存される。
  import { CUSTOM_PROVIDER_ID, PROVIDERS, providerForEmail } from "../lib/providers";
  import { store } from "../lib/store.svelte";
  import type { ServerEndpoint } from "../lib/types";
  import Dialog from "./Dialog.svelte";

  let displayName = $state("");
  let email = $state("");
  let password = $state("");
  let username = $state("");
  let providerId = $state(CUSTOM_PROVIDER_ID);
  let imap = $state<ServerEndpoint>({ host: "", port: 993, security: "tls" });
  let smtp = $state<ServerEndpoint>({ host: "", port: 465, security: "tls" });

  // 利用者が自分で変えた項目は、自動設定で上書きしない
  let usernameEdited = false;
  let providerChosen = false;

  let busy = $state(false);
  let error = $state<string | null>(null);

  const provider = $derived(PROVIDERS.find((candidate) => candidate.id === providerId));
  const complete = $derived(
    email.includes("@") && password !== "" && username !== "" && imap.host !== "" && smtp.host !== "",
  );

  function applyProvider(id: string) {
    const selected = PROVIDERS.find((candidate) => candidate.id === id);
    if (!selected) return;
    providerId = id;
    if (id !== CUSTOM_PROVIDER_ID) {
      imap = { ...selected.imap };
      smtp = { ...selected.smtp };
    }
  }

  function onEmailInput() {
    if (!usernameEdited) username = email.trim();
    const detected = providerForEmail(email);
    if (detected && !providerChosen) applyProvider(detected.id);
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (!complete || busy) return;
    busy = true;
    error = null;
    try {
      await store.addAccount({
        displayName: displayName.trim() || email.split("@")[0],
        email: email.trim(),
        username: username.trim(),
        password,
        imap: { ...imap, host: imap.host.trim() },
        smtp: { ...smtp, host: smtp.host.trim() },
      });
    } catch (cause) {
      error = String(cause);
    } finally {
      busy = false;
    }
  }
</script>

{#snippet endpoint(label: string, server: ServerEndpoint)}
  <fieldset>
    <legend>{label}</legend>
    <input class="host" placeholder="ホスト名" bind:value={server.host} autocapitalize="off" spellcheck="false" />
    <input class="port" type="number" min="1" max="65535" aria-label="ポート" bind:value={server.port} />
    <select aria-label="暗号化" bind:value={server.security}>
      <option value="tls">SSL/TLS</option>
      <option value="startTls">STARTTLS</option>
    </select>
  </fieldset>
{/snippet}

<Dialog title="アカウントを追加" onclose={() => (store.accountDialogOpen = false)}>
  <form id="account-form" onsubmit={submit}>
    <label>
      <span>名前</span>
      <input bind:value={displayName} placeholder="差出人として表示される名前" />
    </label>
    <label>
      <span>メールアドレス</span>
      <!-- svelte-ignore a11y_autofocus : 開いたらすぐ入力を始められるように -->
      <input type="email" bind:value={email} oninput={onEmailInput} autofocus autocapitalize="off" spellcheck="false" />
    </label>
    <label>
      <span>パスワード</span>
      <input type="password" bind:value={password} />
    </label>
    {#if provider?.hint}
      <p class="hint muted">{provider.hint}</p>
    {/if}
    <p class="hint muted">
      パスワードは OS のキーチェーンに保存されます。メール本体はこのコンピューターに保存されません。
    </p>

    <label>
      <span>サービス</span>
      <select
        value={providerId}
        onchange={(event) => {
          providerChosen = true;
          applyProvider(event.currentTarget.value);
        }}
      >
        {#each PROVIDERS as candidate (candidate.id)}
          <option value={candidate.id}>{candidate.label}</option>
        {/each}
      </select>
    </label>

    <details open={providerId === CUSTOM_PROVIDER_ID}>
      <summary>サーバー設定</summary>
      <label>
        <span>ユーザー名</span>
        <input bind:value={username} oninput={() => (usernameEdited = true)} autocapitalize="off" spellcheck="false" />
      </label>
      {@render endpoint("受信 (IMAP)", imap)}
      {@render endpoint("送信 (SMTP)", smtp)}
    </details>

    {#if error}
      <p class="error selectable">{error}</p>
    {/if}
  </form>

  {#snippet footer()}
    <button class="primary" type="submit" form="account-form" disabled={!complete || busy}>
      {busy ? "接続を確認中…" : "追加"}
    </button>
    <button type="button" onclick={() => (store.accountDialogOpen = false)}>キャンセル</button>
  {/snippet}
</Dialog>

<style>
  form {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  label {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  label span {
    width: 96px;
    color: var(--text-muted);
  }

  label input,
  label select {
    flex: 1;
    min-width: 0;
  }

  .hint {
    margin: 0 0 0 104px;
    font-size: 12px;
  }

  details {
    display: flex;
    flex-direction: column;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }

  details[open] summary {
    margin-bottom: 10px;
  }

  summary {
    cursor: pointer;
    color: var(--text-muted);
  }

  fieldset {
    display: flex;
    gap: 6px;
    margin: 10px 0 0;
    padding: 0;
    border: none;
  }

  legend {
    padding: 0;
    margin-bottom: 4px;
    color: var(--text-muted);
    font-size: 12px;
  }

  .host {
    flex: 1;
    min-width: 0;
  }

  .port {
    width: 76px;
  }

  .error {
    margin: 0;
    padding: 8px 10px;
    border-radius: 6px;
    background: var(--accent-soft);
    color: var(--danger);
    overflow-wrap: anywhere;
  }
</style>
