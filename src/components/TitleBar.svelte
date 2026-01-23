<script>
    import { getCurrentWindow } from "@tauri-apps/api/window";
    import { Minus, Square, X } from "lucide-svelte";

    const appWindow = getCurrentWindow();

    function minimize() {
        appWindow.minimize();
    }

    async function toggleMaximize() {
        await appWindow.toggleMaximize();
    }

    function close() {
        appWindow.close();
    }
</script>

<div class="titlebar" data-tauri-drag-region>
    <div class="drag-handle" data-tauri-drag-region></div>
    <div class="controls">
        <button class="titlebar-button" on:click={minimize} title="Minimize">
            <Minus size={16} />
        </button>
        <button
            class="titlebar-button"
            on:click={toggleMaximize}
            title="Maximize"
        >
            <Square size={14} />
        </button>
        <button class="titlebar-button close" on:click={close} title="Close">
            <X size={16} />
        </button>
    </div>
</div>

<style>
    .titlebar {
        height: 32px;
        display: flex;
        justify-content: space-between;
        position: fixed;
        top: 0;
        left: 0;
        right: 0;
        z-index: 99999;
        /* Background is transparent to show content/hero behind it */
    }

    .drag-handle {
        flex-grow: 1;
        height: 100%;
    }

    .controls {
        display: flex;
        -webkit-app-region: no-drag; /* For safety, though buttons usually capture events */
    }

    .titlebar-button {
        display: inline-flex;
        justify-content: center;
        align-items: center;
        width: 46px;
        height: 32px;
        background: transparent;
        border: none;
        color: var(--text-muted, #aaa);
        cursor: default;
        transition:
            background 0.2s,
            color 0.2s;
        outline: none;
    }

    .titlebar-button:hover {
        background: rgba(255, 255, 255, 0.1);
        color: var(--text, #fff);
    }

    .titlebar-button.close:hover {
        background: #e81123;
        color: white;
    }
</style>
