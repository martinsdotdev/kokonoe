<script lang="ts">
	import './layout.css';
	import favicon from '#lib/assets/favicon.svg';
	import { ModeWatcher } from 'mode-watcher';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();
</script>

<!-- Every page also sets its own title: one a page set stays when it's left, until the next sets one. -->
<svelte:head>
	<link rel="icon" href={favicon} />
	<title>Kokonoe</title>
</svelte:head>

<ModeWatcher />
{@render children()}
