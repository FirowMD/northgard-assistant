<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { Accordion } from '@skeletonlabs/skeleton-svelte';

  let displayText = $state("");
  let pid = '';
  const value = $state(['advanced']);

  displayText = "Launcher is up-to-date";

  async function handlePlay() {
    try {
      await invoke("launch_northgard");
    } catch (e) {
      console.error("Failed to start game:", e);
    }
  }

  async function handleAttach() {
    if (!pid) return;
    
    try {
      await invoke("attach_to_pid", { pid: parseInt(pid, 10) });
      displayText = `Successfully attached to PID: ${pid}`;
    } catch (e) {
      console.error('Failed to attach:', e);
      displayText = `Failed to attach to PID: ${pid}`;
    }
  }
</script>


<main class="h-screen p-4">
  <div class="w-[600px] space-y-4 mx-auto mt-8">
    <div class="flex gap-2">
      <input
        type="text"
        class="input w-5/6 p-2"
        placeholder="Launcher is up-to-date"
        bind:value={displayText}
        readonly
      />
      <button 
        type="button"
        class="btn w-1/6 h-full p-2 preset-tonal-tertiary"
        onclick={handlePlay}
      >
        Play
      </button>
    </div>
    <Accordion {value} multiple>
      <Accordion.Item value="advanced">
        {#snippet control()}Advanced{/snippet}
        {#snippet panel()}
          <div class="input-group grid-cols-[1fr_auto] divide-surface-200-800 divide-x">
            <input
              type="text"
              class="input p-2"
              placeholder="Enter PID"
              bind:value={pid}
            />
            <button 
              type="button"
              class="btn preset-filled"
              onclick={handleAttach}
            >
              Attach
            </button>
          </div>
        {/snippet}
      </Accordion.Item>
    </Accordion>
  </div>
</main>