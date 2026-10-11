<script lang="ts">
  // 開いているメールの「送信元がどれだけ確かめられたか」を示す帯。
  // 色とアイコンで一目で分かるようにし、押すと根拠 (DKIM / SPF / DMARC / BIMI) を開く。
  import { store } from "../lib/store.svelte";
  import {
    POLICY_LABELS,
    STATUS_LABELS,
    TRUST_ICONS,
    TRUST_LABELS,
    trustExplanation,
  } from "../lib/trust";
  import Icon from "./Icon.svelte";

  const auth = $derived(store.senderAuth);

  let expanded = $state(false);
  $effect(() => {
    void store.selectedUid;
    expanded = false;
  });
</script>

<div class="row">
  {#if auth}
    <button
      class="badge {auth.level}"
      aria-expanded={expanded}
      title="検証の内訳を表示"
      onclick={() => (expanded = !expanded)}
    >
      <Icon name={TRUST_ICONS[auth.level]} size={15} />
      <span class="label">{TRUST_LABELS[auth.level]}</span>
      {#if auth.fromDomain}
        <span class="domain">{auth.fromDomain}</span>
      {/if}
      <span class="chevron" class:open={expanded}><Icon name="chevronDown" size={13} /></span>
    </button>
  {:else if store.senderAuthLoading}
    <span class="badge pending"><Icon name="shieldQuestion" size={15} /> 送信元を検証中…</span>
  {/if}

  <span class="spacer"></span>
  <button class="ghost source" title="ヘッダーとソースを表示" onclick={() => (store.sourceDialogOpen = true)}>
    <Icon name="code" size={14} /> ソース
  </button>
</div>

{#if auth && expanded}
  <section class="details {auth.level}">
    <p>{trustExplanation(auth)}</p>

    <h3>rumble による検算 (DKIM 署名)</h3>
    {#if auth.signatures.length === 0}
      <p class="muted">このメールには電子署名がありません。</p>
    {:else}
      <ul>
        {#each auth.signatures as signature}
          <li>
            <span class="status {signature.status}">{STATUS_LABELS[signature.status]}</span>
            <span>
              <strong>{signature.domain}</strong>
              <span class="muted">
                ({signature.aligned ? "差出人ドメインの署名" : "差出人とは別のドメイン"} ・ セレクター {signature.selector})
              </span>
              {#if signature.detail}<br /><span class="muted">{signature.detail}</span>{/if}
            </span>
          </li>
        {/each}
      </ul>
    {/if}
    {#if auth.lookupFailed}
      <p class="muted">DNS に問い合わせられなかったため、検算できていない項目があります。</p>
    {/if}

    <h3>
      受信サーバーの申告
      {#if auth.reported}<span class="muted">— {auth.reported.verifier}</span>{/if}
    </h3>
    {#if !auth.reported || auth.reported.checks.length === 0}
      <p class="muted">受信サーバーの検査記録 (Authentication-Results) はありません。</p>
    {:else}
      <ul>
        {#each auth.reported.checks as check}
          <li>
            <span class="status {check.status}">{STATUS_LABELS[check.status]}</span>
            <span>
              <strong>{check.method.toUpperCase()}</strong>
              {check.result}
              {#if check.domain}<span class="muted">({check.domain})</span>{/if}
            </span>
          </li>
        {/each}
      </ul>
      <p class="muted">
        サーバーが受信時に記録した内容で、rumble は検算していません。SPF は受信サーバーにしか検査できません。
      </p>
    {/if}

    <h3>ドメインの方針 (DMARC)</h3>
    <p class={auth.dmarcPolicy ? "" : "muted"}>
      {auth.dmarcPolicy
        ? POLICY_LABELS[auth.dmarcPolicy]
        : "方針を公開していないか、取得できませんでした。"}
    </p>

    {#if auth.logo}
      <h3>ブランドロゴ (BIMI)</h3>
      <p class="logo">
        <img src={auth.logo.dataUrl} alt="" />
        <span>
          {auth.logo.domain} が自分のドメインで公開しているロゴです。
          <span class="muted">
            {auth.logo.hasCertificate
              ? "商標の証明書 (VMC) が添えられていますが、rumble は証明書を検証していません。"
              : "商標の証明書 (VMC) は添えられていません。"}
          </span>
        </span>
      </p>
    {/if}
  </section>
{/if}

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 10px;
  }

  .spacer {
    flex: 1;
  }

  .badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    padding: 4px 10px;
    border: 1px solid currentColor;
    border-radius: 999px;
    background: color-mix(in srgb, currentColor 10%, transparent);
    font-weight: 600;
  }

  /* 判定ごとの色。帯・内訳の左線・合否の文字で共通に使う */
  .verified {
    color: var(--trust-verified);
  }

  .reported {
    color: var(--trust-reported);
  }

  .partial {
    color: var(--trust-partial);
  }

  .failed {
    color: var(--danger);
  }

  .unknown,
  .pending {
    color: var(--text-muted);
  }

  .pending {
    font-weight: 400;
  }

  .domain {
    overflow: hidden;
    font-weight: 400;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .label {
    white-space: nowrap;
  }

  .chevron {
    display: flex;
    transition: transform 0.15s;
  }

  .chevron.open {
    transform: rotate(180deg);
  }

  .source {
    flex: none;
    color: var(--text-muted);
  }

  .details {
    /* 内訳が長くても本文を画面の外へ押し出さない */
    max-height: 40vh;
    overflow-y: auto;
    margin-top: 10px;
    padding: 10px 12px;
    border-left: 3px solid currentColor;
    border-radius: 0 6px 6px 0;
    background: var(--bg-list);
  }

  /* 左線だけ判定の色にし、中の文章は通常の文字色に戻す */
  .details > * {
    color: var(--text);
  }

  .details > .muted,
  .details .muted {
    color: var(--text-muted);
  }

  p {
    margin: 0;
  }

  h3 {
    margin: 12px 0 4px;
    font-size: 12px;
  }

  h3 .muted {
    font-weight: 400;
  }

  ul {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 0 0 4px;
    padding: 0;
    list-style: none;
  }

  li {
    display: flex;
    align-items: baseline;
    gap: 8px;
    overflow-wrap: anywhere;
  }

  .status {
    flex: none;
    width: 5.5em;
    font-weight: 600;
  }

  .status.pass {
    color: var(--trust-verified);
  }

  .status.fail {
    color: var(--danger);
  }

  .status.inconclusive,
  .status.none {
    color: var(--text-muted);
  }

  .logo {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .logo img {
    flex: none;
    width: 40px;
    height: 40px;
    border: 1px solid var(--border);
    border-radius: 50%;
    background: #fff;
  }
</style>
