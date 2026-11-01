<!-- An invite Room's link, keyed by the Room so another Room's link starts afresh. -->
<script lang="ts">
	import { page } from '$app/state';
	import RoomScreen from '#lib/room/RoomScreen.svelte';
</script>

{#key page.params.room}
	<RoomScreen room={page.params.room ?? ''} />
{/key}
