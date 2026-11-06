<script lang="ts">
	// The one memorable element: the wordmark styled like a European number
	// plate, with a blue strip on the left.
	let { size = 'md' }: { size?: 'md' | 'lg' } = $props();
</script>

<span class="plate {size}" aria-label="Autokarta">
	<span class="strip" aria-hidden="true">
		<svg viewBox="0 0 20 20" class="ring">
			{#each Array.from({ length: 12 }, (_, i) => i) as i (i)}
				<circle
					cx={10 + 7 * Math.cos((i * Math.PI) / 6)}
					cy={10 + 7 * Math.sin((i * Math.PI) / 6)}
					r="1.1"
				/>
			{/each}
		</svg>
		<span class="code">AK</span>
	</span>
	<span class="text" aria-hidden="true">AUTOKARTA</span>
</span>

<style>
	.plate {
		display: inline-flex;
		align-items: stretch;
		background: var(--plate);
		color: var(--plate-ink);
		border: 2px solid var(--plate-ink);
		border-radius: 5px;
		overflow: hidden;
		line-height: 1;
		user-select: none;
	}
	.strip {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 2px;
		background: #1f4e9c;
		color: #fff;
		padding: 3px 4px;
	}
	.ring {
		width: 14px;
		height: 14px;
		fill: #f2c94c;
	}
	.code {
		font-family: var(--font-head);
		font-weight: 700;
		font-size: 9px;
		letter-spacing: 0.04em;
	}
	.text {
		display: flex;
		align-items: center;
		padding: 0 9px;
		font-family: var(--font-head);
		font-weight: 700;
		font-size: 21px;
		letter-spacing: 0.08em;
	}
	.lg .strip {
		padding: 6px 7px;
		gap: 3px;
	}
	.lg .ring {
		width: 22px;
		height: 22px;
	}
	.lg .code {
		font-size: 13px;
	}
	.lg .text {
		font-size: 34px;
		padding: 2px 14px;
	}
</style>
