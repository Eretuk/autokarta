import { sveltekit } from '@sveltejs/kit/vite';
import { SvelteKitPWA } from '@vite-pwa/sveltekit';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [
		sveltekit(),
		SvelteKitPWA({
			registerType: 'autoUpdate',
			// Registered manually in the root layout via virtual:pwa-register.
			injectRegister: false,
			kit: { adapterFallback: 'index.html', spa: true },
			manifest: {
				name: 'Autokarta',
				short_name: 'Autokarta',
				description: 'Workshop, parts and fleet management',
				start_url: '/',
				scope: '/',
				display: 'standalone',
				background_color: '#e7ece6',
				theme_color: '#1f4e9c',
				icons: [
					{ src: '/icons/icon-192.png', sizes: '192x192', type: 'image/png' },
					{ src: '/icons/icon-512.png', sizes: '512x512', type: 'image/png' },
					{
						src: '/icons/icon-maskable-512.png',
						sizes: '512x512',
						type: 'image/png',
						purpose: 'maskable'
					}
				]
			},
			workbox: {
				// The app shell is cached so it opens fast; API data is never cached
				// (offline mode is out of scope).
				globPatterns: ['**/*.{js,css,html,svg,png,woff2,wasm,webmanifest}'],
				navigateFallbackDenylist: [/^\/api\//],
				// zxing_reader.wasm is about 1 MB.
				maximumFileSizeToCacheInBytes: 4 * 1024 * 1024
			}
		})
	],
	server: {
		proxy: {
			'/api': 'http://localhost:8080'
		}
	}
});
