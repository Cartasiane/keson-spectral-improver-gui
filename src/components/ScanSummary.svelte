<script>
  import { createEventDispatcher } from "svelte";
  import { Check, AlertCircle, HelpCircle, BadgeCheck } from "lucide-svelte";

  export let results = [];
  export let active = "bad";
  export let downloadedCount = 0;
  export let noMatchCount = 0;
  export let reviewCount = 0;
  export let replacedCount = 0;

  const dispatch = createEventDispatcher();

  $: total = results.length;
  $: bad = results.filter((r) => r.status === "bad").length;
  $: ok = results.filter((r) => r.status === "ok").length;
  $: replaced = results.filter(
    (r) => r.status === "replaced" || r.replaced,
  ).length;
  $: err = results.filter((r) => r.status === "error").length;

  function setFilter(val) {
    dispatch("filter", val);
  }
</script>

<div class="scan-summary">
  <button
    class={`pill ghost ${active === "all" ? "active" : ""}`}
    on:click={() => setFilter("all")}
  >
    Total {total}
  </button>
  <button
    class={`pill warn ${active === "bad" ? "active" : ""}`}
    on:click={() => setFilter("bad")}
  >
    Low {bad}
  </button>
  <button
    class={`pill ${active === "ok" ? "active" : ""}`}
    on:click={() => setFilter("ok")}
  >
    OK {ok}
  </button>
  {#if replaced > 0 || replacedCount > 0}
    <button
      class={`pill replaced ${active === "replaced" ? "active" : ""}`}
      on:click={() => setFilter("replaced")}
    >
      <BadgeCheck size={16} style="margin-right: 4px" />Déjà spectralisé {replaced ||
        replacedCount}
    </button>
  {/if}
  {#if downloadedCount > 0}
    <button
      class={`pill success ${active === "downloaded" ? "active" : ""}`}
      on:click={() => setFilter("downloaded")}
    >
      <Check size={16} /> Downloaded {downloadedCount}
    </button>
  {/if}
  {#if reviewCount > 0}
    <button
      class={`pill review ${active === "review" ? "active" : ""}`}
      on:click={() => setFilter("review")}
    >
      <HelpCircle size={16} style="margin-right: 4px" /> À vérifier {reviewCount}
    </button>
  {/if}
  {#if noMatchCount > 0}
    <button
      class={`pill nomatch ${active === "no-match" ? "active" : ""}`}
      on:click={() => setFilter("no-match")}
    >
      <AlertCircle size={16} style="margin-right: 4px" /> Non trouvés {noMatchCount}
    </button>
  {/if}
  <button
    class={`pill ${active === "error" ? "active" : ""}`}
    on:click={() => setFilter("error")}
  >
    Err {err}
  </button>
</div>

<style>
  .scan-summary {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .pill.active {
    outline: 2px solid currentColor;
    box-shadow: 0 0 0 2px rgba(0, 0, 0, 0.08);
    transform: translateY(-1px);
  }
  .pill.warn.active {
    outline-color: var(--accent);
  }
  .pill.success {
    background: linear-gradient(135deg, #10b981, #059669);
    color: white;
  }
  .pill.success.active {
    outline-color: #10b981;
  }
  .pill.review {
    background: linear-gradient(135deg, #3b82f6, #2563eb);
    color: white;
  }
  .pill.review.active {
    outline-color: #3b82f6;
  }
  .pill.nomatch {
    background: linear-gradient(135deg, #f59e0b, #d97706);
    color: white;
  }
  .pill.nomatch.active {
    outline-color: #f59e0b;
  }
  .pill.replaced {
    background: linear-gradient(135deg, #8b5cf6, #7c3aed);
    color: white;
  }
  .pill.replaced.active {
    outline-color: #8b5cf6;
  }
</style>
