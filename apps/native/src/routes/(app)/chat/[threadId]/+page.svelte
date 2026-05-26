<script lang="ts">
  import { page } from "$app/state";

  let threadId = $derived(page.params.threadId);
</script>

<p>Thread: {threadId}</p>
