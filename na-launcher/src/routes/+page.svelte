<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { Accordion } from '@skeletonlabs/skeleton-svelte';
  import ProcessAttacher from '../lib/ProcessAttacher.svelte';

  let displayText = $state("");
  const value = $state(['advanced']);

  displayText = "Launcher is up-to-date";

  async function handlePlay() {
    try {
      await invoke("launch_northgard");
    } catch (e) {
      console.error("Failed to start game:", e);
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
        <Accordion.ItemTrigger class="flex justify-between items-center">
          Advanced
        </Accordion.ItemTrigger>
        <Accordion.ItemContent>
          <div class="space-y-2">
            <ProcessAttacher on:status={(e) => (displayText = e.detail)} />
          </div>
        </Accordion.ItemContent>
      </Accordion.Item>
    </Accordion>
  </div>
</main>