<script>
  import { createEventDispatcher } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  export let show = false;
  export let slotsRemaining = null;

  let inviteCode = "";
  let loading = false;
  let error = "";

  const dispatch = createEventDispatcher();

  async function handleSubmit() {
    if (!inviteCode.trim()) {
      error = "Veuillez entrer un code d'invitation.";
      return;
    }

    loading = true;
    error = "";

    try {
      await invoke("register_client", {
        inviteCode: inviteCode.trim().toUpperCase(),
      });
      dispatch("registered");
    } catch (err) {
      error =
        typeof err === "string"
          ? err
          : err?.message || "Échec de l'enregistrement";
    } finally {
      loading = false;
    }
  }

  function handleKeydown(event) {
    if (event.key === "Enter" && !loading) {
      handleSubmit();
    }
  }
</script>

{#if show}
  <div class="modal-backdrop" />
  <div class="modal registration-modal">
    <div class="modal-head">
      <h2>🔐 Enregistrement requis</h2>
    </div>

    <div class="modal-body">
      <p class="desc">
        Pour utiliser Keson, vous devez entrer un code d'invitation. Demandez-le
        à un administrateur.
      </p>

      {#if slotsRemaining !== null && slotsRemaining > 0}
        <p class="slots-info">
          {slotsRemaining} place{slotsRemaining > 1 ? "s" : ""} restante{slotsRemaining >
          1
            ? "s"
            : ""} avec ce code
        </p>
      {/if}

      <label>
        <span>Code d'invitation</span>
        <input
          type="text"
          placeholder="ABC123"
          bind:value={inviteCode}
          on:keydown={handleKeydown}
          disabled={loading}
          class="invite-input"
          maxlength="10"
        />
      </label>

      {#if error}
        <p class="error">{error}</p>
      {/if}
    </div>

    <div class="actions">
      <button class="btn primary" on:click={handleSubmit} disabled={loading}>
        {loading ? "Enregistrement..." : "S'enregistrer"}
      </button>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.8);
    z-index: 1000;
  }

  .registration-modal {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    background: var(--surface-1, #1a1a2e);
    border: 1px solid var(--border, #333);
    border-radius: 16px;
    padding: 2rem;
    z-index: 1001;
    min-width: 360px;
    max-width: 90vw;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
  }

  .modal-head {
    margin-bottom: 1.5rem;
    text-align: center;
  }

  .modal-head h2 {
    margin: 0;
    font-size: 1.5rem;
    color: var(--primary, #646cff);
  }

  .modal-body {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .desc {
    color: var(--text-muted, #888);
    font-size: 0.9rem;
    text-align: center;
    margin: 0;
    line-height: 1.5;
  }

  .slots-info {
    text-align: center;
    font-size: 0.8rem;
    color: var(--primary, #646cff);
    margin: 0;
    padding: 0.5rem;
    background: rgba(100, 108, 255, 0.1);
    border-radius: 8px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  label span {
    font-size: 0.85rem;
    font-weight: 500;
    color: var(--text-muted, #888);
  }

  .invite-input {
    background: var(--surface-2, #252540);
    border: 2px solid var(--border, #333);
    padding: 1rem;
    border-radius: 12px;
    color: inherit;
    font-size: 1.5rem;
    text-align: center;
    letter-spacing: 0.3em;
    text-transform: uppercase;
    font-family: monospace;
    transition: border-color 0.2s;
  }

  .invite-input:focus {
    border-color: var(--primary, #646cff);
    outline: none;
  }

  .invite-input::placeholder {
    letter-spacing: 0.2em;
    opacity: 0.4;
  }

  .error {
    color: #ff6b6b;
    font-size: 0.9rem;
    margin: 0;
    text-align: center;
    padding: 0.5rem;
    background: rgba(255, 107, 107, 0.1);
    border-radius: 8px;
  }

  .actions {
    margin-top: 1.5rem;
    display: flex;
    justify-content: center;
  }

  .btn.primary {
    padding: 0.75rem 2rem;
    font-size: 1rem;
    min-width: 180px;
  }
</style>
