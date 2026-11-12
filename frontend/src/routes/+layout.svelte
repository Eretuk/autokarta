<script lang="ts">
	import '@fontsource/barlow/latin-400.css';
	import '@fontsource/barlow/latin-ext-400.css';
	import '@fontsource/barlow/latin-600.css';
	import '@fontsource/barlow/latin-ext-600.css';
	import '@fontsource/barlow-condensed/latin-700.css';
	import '@fontsource/barlow-condensed/latin-ext-700.css';
	import '../app.css';

	import { onMount } from 'svelte';
	import { redirectToLogin, setUnauthorizedHandler } from '$lib/api';
	import { i18n } from '$lib/i18n/index.svelte';
	import { registerServiceWorker } from '$lib/pwa.svelte';
	import { session } from '$lib/session.svelte';

	let { children } = $props();

	setUnauthorizedHandler(() => {
		session.user = null;
		redirectToLogin();
	});

	$effect(() => {
		document.documentElement.lang = i18n.locale;
	});

	onMount(() => {
		if (import.meta.env.PROD) registerServiceWorker();
	});
</script>

{@render children()}
