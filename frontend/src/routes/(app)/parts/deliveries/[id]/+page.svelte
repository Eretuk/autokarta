<script lang="ts">
	// One delivery (purchase invoice): header, lines, files. A draft changes
	// nothing on stock; "Post to stock" adds every line at its net cost and
	// can update shelf prices in the same step.
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { onMount, tick } from 'svelte';
	import { api, errorMessage, uploadFile } from '$lib/api';
	import PartPicker from '$lib/components/PartPicker.svelte';
	import Sheet from '$lib/components/Sheet.svelte';
	import SupplierPicker from '$lib/components/SupplierPicker.svelte';
	import { formatMoney, formatQuantity, moneyInput, parseMoney, parseQuantity, quantityInput } from '$lib/format';
	import { formatDate, formatNumber, t } from '$lib/i18n/index.svelte';
	import type { DeliveryDetail, DeliveryItem, DocumentType, PartDetail, PartSummary, PartUnit, Supplier } from '$lib/types';

	const DOC_TYPES: DocumentType[] = ['invoice', 'receipt', 'none'];

	let { data } = $props();
	let d = $derived<DeliveryDetail>(data.delivery);

	const currency = $derived(d.currency);
	const draft = $derived(d.status === 'draft');
	const base = $derived(`/deliveries/${d.id}`);
	const here = $derived(`/parts/deliveries/${d.id}`);
	const unmatched = $derived(d.items.filter((i) => !i.part_id).length);
	const mismatch = $derived(
		d.invoice_total_net_minor !== null && d.items.length > 0 && d.invoice_total_net_minor !== d.totals.net_minor
	);

	let error = $state('');

	/** An invoice goes on stock only with its issuer and number (as saved). */
	const missing = $derived(
		d.document_type === 'invoice'
			? [
					...(d.supplier_id ? [] : [t('deliveries.supplier')]),
					...(d.invoice_number ? [] : [t('deliveries.invoiceNumber')])
				]
			: []
	);

	// ----- header -----
	let docType = $state<DocumentType>('invoice');
	let supplier = $state<Supplier | null>(null);
	let invoiceNumber = $state('');
	let invoiceDate = $state('');
	let invoiceTotal = $state('');
	let note = $state('');
	let hBusy = $state(false);
	let hMsg = $state<{ ok: boolean; text: string } | null>(null);

	function fillHeader(x: DeliveryDetail) {
		docType = x.document_type;
		supplier = x.supplier_id
			? { id: x.supplier_id, name: x.supplier_name ?? '', tax_id: x.supplier_tax_id, note: null }
			: null;
		invoiceNumber = x.invoice_number ?? '';
		invoiceDate = x.invoice_date;
		invoiceTotal = moneyInput(x.invoice_total_net_minor, x.currency);
		note = x.note ?? '';
	}
	// Again when the page moves to another delivery.
	$effect.pre(() => fillHeader(data.delivery));

	async function saveHeader(e: SubmitEvent) {
		e.preventDefault();
		let total: number | null = null;
		if (invoiceTotal.trim()) {
			total = parseMoney(invoiceTotal, currency);
			if (total === null) {
				hMsg = { ok: false, text: t('errors.invalid_price') };
				return;
			}
		}
		hBusy = true;
		hMsg = null;
		try {
			d = await api<DeliveryDetail>('PATCH', base, {
				document_type: docType,
				supplier_id: supplier?.id ?? null,
				invoice_number: docType === 'none' ? null : invoiceNumber.trim() || null,
				invoice_date: invoiceDate || undefined,
				invoice_total_net_minor: docType === 'none' ? null : total,
				note: note.trim() || null
			});
			fillHeader(d);
			hMsg = { ok: true, text: t('deliveries.headerSaved') };
		} catch (err) {
			hMsg = { ok: false, text: errorMessage(err) };
		} finally {
			hBusy = false;
		}
	}

	// ----- lines -----
	type LinePart = { id: string; name: string; brand: string | null; part_number: string | null; unit: PartUnit };
	let lineOpen = $state(false);
	/** Editing this line; null: adding a new one. */
	let lineId = $state<string | null>(null);
	let picking = $state(false);
	let linePart = $state<LinePart | null>(null);
	let lDesc = $state('');
	let lArticle = $state('');
	let lQty = $state('1');
	let lCost = $state('');
	let lVat = $state('');
	let lError = $state('');
	let lBusy = $state(false);
	let lQtyInput = $state<HTMLInputElement>();

	function resetLine() {
		lineId = null;
		linePart = null;
		lDesc = '';
		lArticle = '';
		lQty = '1';
		lCost = '';
		lVat = String(d.default_vat_percent);
		lError = '';
	}

	function addWithPart() {
		resetLine();
		picking = true;
		lineOpen = true;
	}

	function addWithoutPart() {
		resetLine();
		picking = false;
		lineOpen = true;
	}

	function editLine(i: DeliveryItem) {
		resetLine();
		lineId = i.id;
		linePart = i.part_id
			? {
					id: i.part_id,
					name: i.part_name ?? '',
					brand: i.part_brand,
					part_number: i.part_part_number,
					unit: i.part_unit ?? 'pcs'
				}
			: null;
		lDesc = i.description ?? '';
		lArticle = i.part_number ?? '';
		lQty = quantityInput(i.quantity_milli);
		lCost = moneyInput(i.unit_cost_minor, currency);
		lVat = String(i.vat_percent);
		picking = false;
		lineOpen = true;
	}

	/** For an unmatched line: go straight to the part search. */
	function matchLine(i: DeliveryItem) {
		editLine(i);
		picking = true;
	}

	async function pickPart(p: PartSummary) {
		linePart = { id: p.id, name: p.name, brand: p.brand, part_number: p.part_number, unit: p.unit };
		picking = false;
		// A new line starts with the part's last cost, the most likely one.
		if (!lineId && !lCost.trim()) {
			try {
				const full = await api<PartDetail>('GET', `/parts/${p.id}`);
				if (full.last_cost_minor !== undefined && full.last_cost_minor !== null) {
					lCost = moneyInput(full.last_cost_minor, currency);
				}
			} catch {
				// Only a convenience.
			}
		}
		await tick();
		lQtyInput?.focus();
		lQtyInput?.select();
	}

	async function saveLine(e: SubmitEvent) {
		e.preventDefault();
		const quantity = parseQuantity(lQty);
		if (quantity === null) {
			lError = t('errors.invalid_quantity');
			return;
		}
		const cost = parseMoney(lCost, currency);
		if (cost === null) {
			lError = t('errors.invalid_price');
			return;
		}
		const vat = Number(lVat.trim());
		if (!/^\d{1,3}$/.test(lVat.trim()) || vat > 100) {
			lError = t('errors.invalid_vat');
			return;
		}
		if (!linePart && !lDesc.trim()) {
			lError = t('errors.invalid_description');
			return;
		}
		const body = {
			part_id: linePart?.id ?? null,
			description: lDesc.trim() || null,
			part_number: lArticle.trim() || null,
			quantity_milli: quantity,
			unit_cost_minor: cost,
			vat_percent: vat
		};
		lBusy = true;
		lError = '';
		try {
			d = lineId
				? await api<DeliveryDetail>('PATCH', `${base}/items/${lineId}`, body)
				: await api<DeliveryDetail>('POST', `${base}/items`, body);
			lineOpen = false;
		} catch (err) {
			lError = errorMessage(err);
		} finally {
			lBusy = false;
		}
	}

	async function deleteLine() {
		if (!lineId || !confirm(t('deliveries.confirmDeleteLine'))) return;
		lBusy = true;
		lError = '';
		try {
			d = await api<DeliveryDetail>('DELETE', `${base}/items/${lineId}`);
			lineOpen = false;
		} catch (err) {
			lError = errorMessage(err);
		} finally {
			lBusy = false;
		}
	}

	/** Creating a part for an unmatched line, prefilled from the invoice. */
	function createPartHref(i: DeliveryItem): string {
		const params = new URLSearchParams();
		if (i.description) params.set('name', i.description);
		if (i.part_number) params.set('part_number', i.part_number);
		params.set('back', `${here}?line=${i.id}`);
		return `/parts/new?${params}`;
	}

	// Coming back from "New part": attach it to the line it was made for,
	// or start a new line with it.
	onMount(async () => {
		const created = page.url.searchParams.get('created');
		const line = page.url.searchParams.get('line');
		// From "Receive" on a part page: the line just added, open to edit.
		const edit = page.url.searchParams.get('edit');
		if (edit) {
			history.replaceState(history.state, '', here);
			const item = d.items.find((i) => i.id === edit);
			if (item && draft) {
				editLine(item);
				await tick();
				lQtyInput?.focus();
				lQtyInput?.select();
			}
			return;
		}
		if (!created) return;
		history.replaceState(history.state, '', here);
		if (line && draft) {
			try {
				d = await api<DeliveryDetail>('PATCH', `${base}/items/${line}`, { part_id: created });
			} catch (err) {
				error = errorMessage(err);
			}
		} else if (draft) {
			try {
				const p = await api<PartDetail>('GET', `/parts/${created}`);
				resetLine();
				lineOpen = true;
				await pickPart({ ...p, barcode_match: false });
			} catch (err) {
				error = errorMessage(err);
			}
		}
	});

	function lineName(i: DeliveryItem): string {
		return i.part_name ?? i.description ?? '';
	}

	// ----- files -----
	let fileInput = $state<HTMLInputElement>();
	let fBusy = $state(false);
	let fError = $state('');

	async function upload(e: Event) {
		const input = e.currentTarget as HTMLInputElement;
		const files = Array.from(input.files ?? []);
		input.value = '';
		if (files.length === 0) return;
		fBusy = true;
		fError = '';
		try {
			for (const f of files) {
				d = await uploadFile<DeliveryDetail>(`${base}/files?name=${encodeURIComponent(f.name)}`, f);
			}
		} catch (err) {
			fError = errorMessage(err);
		} finally {
			fBusy = false;
		}
	}

	async function deleteFile(id: string, name: string) {
		if (!confirm(t('deliveries.confirmDeleteFile', { name }))) return;
		fError = '';
		try {
			d = await api<DeliveryDetail>('DELETE', `${base}/files/${id}`);
		} catch (err) {
			fError = errorMessage(err);
		}
	}

	function fileSize(bytes: number): string {
		return bytes >= 1024 * 1024
			? t('deliveries.sizeMb', { value: formatNumber(bytes / 1024 / 1024, { maximumFractionDigits: 1 }) })
			: t('deliveries.sizeKb', { value: formatNumber(Math.max(1, Math.round(bytes / 1024))) });
	}

	// ----- post -----
	type PriceRow = {
		part_id: string;
		name: string;
		current: number | null;
		previous: number | null;
		cost: number;
		suggested: number | null;
		update: boolean;
		price: string;
	};
	let postOpen = $state(false);
	let priceRows = $state<PriceRow[]>([]);
	let postError = $state('');
	let postBusy = $state(false);

	function openPost() {
		const seen = new Set<string>();
		priceRows = [];
		for (const i of d.items) {
			if (!i.part_id || seen.has(i.part_id)) continue;
			seen.add(i.part_id);
			const suggested = i.suggested_price_minor ?? null;
			const current = i.part_sell_price_minor;
			const costChanged = i.previous_cost_minor !== i.unit_cost_minor;
			const update = suggested !== null && suggested !== current && (current === null || costChanged);
			priceRows.push({
				part_id: i.part_id,
				name: lineName(i),
				current,
				previous: i.previous_cost_minor,
				cost: i.unit_cost_minor,
				suggested,
				update,
				price: moneyInput(update ? suggested : (current ?? suggested), currency)
			});
		}
		postError = '';
		postOpen = true;
	}

	async function post(e: SubmitEvent) {
		e.preventDefault();
		const prices: { part_id: string; sell_price_minor: number }[] = [];
		for (const r of priceRows) {
			if (!r.update) continue;
			const p = parseMoney(r.price, currency);
			if (p === null) {
				postError = t('errors.invalid_price');
				return;
			}
			prices.push({ part_id: r.part_id, sell_price_minor: p });
		}
		postBusy = true;
		postError = '';
		try {
			d = await api<DeliveryDetail>('POST', `${base}/post`, { prices });
			postOpen = false;
		} catch (err) {
			postError = errorMessage(err);
		} finally {
			postBusy = false;
		}
	}

	let aBusy = $state(false);

	async function reopen() {
		if (!confirm(t('deliveries.confirmReopen'))) return;
		aBusy = true;
		error = '';
		try {
			d = await api<DeliveryDetail>('POST', `${base}/reopen`);
		} catch (err) {
			error = errorMessage(err);
		} finally {
			aBusy = false;
		}
	}

	async function remove() {
		if (!confirm(t('deliveries.confirmDelete'))) return;
		aBusy = true;
		error = '';
		try {
			await api('DELETE', base);
			await goto('/parts/deliveries', { replaceState: true });
		} catch (err) {
			error = errorMessage(err);
		} finally {
			aBusy = false;
		}
	}
</script>

<svelte:head><title>{t('deliveries.number', { number: d.number })} · Autokarta</title></svelte:head>

<p class="crumbs"><a href="/parts/deliveries">← {t('deliveries.title')}</a></p>
<header class="head">
	<h1>{t('deliveries.number', { number: d.number })}</h1>
	<span class="status" class:open={draft} class:done={!draft}>{t(`deliveries.status.${d.status}`)}</span>
</header>
{#if !draft && d.posted_at}
	<p class="hint">
		{t('deliveries.postedInfo', { date: formatDate(d.posted_at), name: d.posted_by_name ?? '—' })}
	</p>
{:else}
	<p class="hint">{t('deliveries.draftHint')}</p>
{/if}

{#if error}<div class="notice error" role="alert">{error}</div>{/if}

<section class="card">
	<h2>{t('deliveries.document')}</h2>
	<form onsubmit={saveHeader}>
		{#if hMsg}
			<div class="notice" class:ok={hMsg.ok} class:error={!hMsg.ok} role="status">{hMsg.text}</div>
		{/if}
		<span class="label" id="doc-type-label">{t('deliveries.documentType')}</span>
		<div class="segmented doc-types" role="group" aria-labelledby="doc-type-label">
			{#each DOC_TYPES as dt (dt)}
				<button type="button" class="btn" aria-pressed={docType === dt} onclick={() => (docType = dt)}
					>{t(`deliveries.docTypes.${dt}`)}</button
				>
			{/each}
		</div>
		<p class="hint">{t(`deliveries.docTypeHints.${docType}`)}</p>
		<SupplierPicker
			bind:selected={supplier}
			label={docType === 'invoice' ? t('deliveries.supplierRequired') : t('deliveries.supplierOptional')}
		/>
		<div class="two-cols">
			{#if docType !== 'none'}
				<label class="field">
					{docType === 'invoice' ? t('deliveries.invoiceNumberRequired') : t('deliveries.receiptNumber')}
					<input type="text" bind:value={invoiceNumber} maxlength="60" autocomplete="off" />
				</label>
			{/if}
			<label class="field">
				{t('deliveries.documentDate')}
				<input type="date" bind:value={invoiceDate} required />
			</label>
		</div>
		{#if docType !== 'none'}
			<label class="field">
				{t('deliveries.invoiceTotal', { currency })}
				<span class="hint">{t('deliveries.invoiceTotalHint')}</span>
				<input type="text" inputmode="decimal" bind:value={invoiceTotal} maxlength="16" autocomplete="off" />
			</label>
		{/if}
		<label class="field">
			{docType === 'none' ? t('deliveries.noteNone') : t('deliveries.note')}
			<input type="text" bind:value={note} maxlength="2000" autocomplete="off" />
		</label>
		<button class="btn" type="submit" disabled={hBusy}>{t('common.saveChanges')}</button>
	</form>
</section>

<div class="section-head"><h2>{t('deliveries.linesTitle')}</h2></div>
{#if unmatched > 0}
	<div class="notice error" role="status">{t('deliveries.unmatchedNotice', { count: unmatched })}</div>
{/if}
{#if d.items.length === 0}
	<p class="hint">{t('deliveries.noLines')}</p>
{:else}
	<ul class="rows">
		{#each d.items as i (i.id)}
			<li class="line">
				<svelte:element
					this={draft ? 'button' : 'div'}
					class="row-link"
					type={draft ? 'button' : undefined}
					onclick={draft ? () => editLine(i) : undefined}
					role={draft ? undefined : 'group'}
				>
					<span class="row-main">
						<span class="name">
							{lineName(i)}
							{#if i.part_condition === 'used'}<span class="badge accent">{t('parts.conditions.used')}</span>{/if}
							{#if !i.part_id}<span class="badge danger">{t('deliveries.unmatched')}</span>{/if}
						</span>
						{#if i.part_brand || i.part_part_number || i.part_number}
							<span class="hint">
								{i.part_brand ?? ''}{#if i.part_brand && (i.part_part_number || i.part_number)}{' · '}{/if}<span class="mono"
									>{i.part_part_number ?? i.part_number ?? ''}</span
								>
							</span>
						{/if}
						<span class="hint">
							{formatQuantity(i.quantity_milli)}{i.part_unit ? ' ' + t(`units.${i.part_unit}`) : ''} × {formatMoney(
								i.unit_cost_minor,
								currency
							)} · {t('deliveries.vatShort', { rate: i.vat_percent })}
						</span>
					</span>
					<span class="row-side">
						<strong>{formatMoney(i.total_net_minor, currency)}</strong>
						{#if i.part_id && i.previous_cost_minor !== null && i.previous_cost_minor !== i.unit_cost_minor}
							<span class="hint" class:up={i.unit_cost_minor > i.previous_cost_minor}
								>{t('deliveries.wasCost', { price: formatMoney(i.previous_cost_minor, currency) })}</span
							>
						{/if}
					</span>
				</svelte:element>
				{#if draft && !i.part_id}
					<div class="match">
						<button class="btn" type="button" onclick={() => matchLine(i)}>{t('deliveries.choosePart')}</button>
						<a class="btn" href={createPartHref(i)}>{t('deliveries.createPart')}</a>
					</div>
				{/if}
			</li>
		{/each}
	</ul>
{/if}

{#if draft}
	<div class="actions add-lines">
		<button class="btn primary" type="button" onclick={addWithPart}>{t('deliveries.addLine')}</button>
		<button class="btn" type="button" onclick={addWithoutPart}>{t('deliveries.addUnmatched')}</button>
	</div>
{/if}

<section class="card totals">
	<dl>
		<dt>{t('deliveries.net')}</dt>
		<dd>{formatMoney(d.totals.net_minor, currency)}</dd>
		<dt>{t('deliveries.vat')}</dt>
		<dd>{formatMoney(d.totals.vat_minor, currency)}</dd>
		<dt><strong>{t('deliveries.gross')}</strong></dt>
		<dd><strong>{formatMoney(d.totals.gross_minor, currency)}</strong></dd>
	</dl>
	{#if mismatch && d.invoice_total_net_minor !== null}
		<div class="notice error" role="status">
			{t('deliveries.totalMismatch', {
				invoice: formatMoney(d.invoice_total_net_minor, currency),
				lines: formatMoney(d.totals.net_minor, currency)
			})}
		</div>
	{/if}
</section>

<section class="card">
	<h2>{t('deliveries.files')}</h2>
	<p class="hint">{t('deliveries.filesHint')}</p>
	{#if fError}<div class="notice error" role="alert">{fError}</div>{/if}
	{#if d.files.length > 0}
		<ul class="files">
			{#each d.files as f (f.id)}
				<li>
					<a href="/api{base}/files/{f.id}" target="_blank" rel="noopener">
						<span class="fname">{f.file_name}</span>
						<span class="hint">{fileSize(f.size_bytes)}</span>
					</a>
					<button
						class="remove"
						type="button"
						aria-label={t('deliveries.deleteFile', { name: f.file_name })}
						onclick={() => deleteFile(f.id, f.file_name)}
					>
						<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18" /></svg>
					</button>
				</li>
			{/each}
		</ul>
	{/if}
	<input
		bind:this={fileInput}
		class="visually-hidden"
		type="file"
		accept="application/pdf,.pdf,application/xml,text/xml,.xml,image/jpeg,image/png,image/webp"
		multiple
		onchange={upload}
		tabindex="-1"
	/>
	<button class="btn block" type="button" disabled={fBusy} onclick={() => fileInput?.click()}>
		{fBusy ? t('deliveries.uploading') : t('deliveries.attach')}
	</button>
</section>

<div class="final">
	{#if draft}
		<button
			class="btn primary block"
			type="button"
			disabled={aBusy || d.items.length === 0 || unmatched > 0 || missing.length > 0}
			onclick={openPost}>{t('deliveries.post')}</button
		>
		{#if missing.length > 0}
			<p class="hint">{t('deliveries.invoiceMissing', { fields: missing.join(', ') })}</p>
		{/if}
		{#if unmatched > 0}<p class="hint">{t('deliveries.postBlocked')}</p>{/if}
		<button class="btn danger block" type="button" disabled={aBusy} onclick={remove}>{t('deliveries.delete')}</button>
	{:else}
		<button class="btn block" type="button" disabled={aBusy} onclick={reopen}>{t('deliveries.reopen')}</button>
	{/if}
</div>

<Sheet bind:open={lineOpen} title={lineId ? t('deliveries.editLine') : t('deliveries.addLine')}>
	{#if picking}
		<PartPicker onPick={pickPart} back={here} />
		{#if lineId || linePart}
			<button class="btn block gap" type="button" onclick={() => (picking = false)}>{t('common.cancel')}</button>
		{/if}
	{:else}
		<form onsubmit={saveLine}>
			{#if lError}<div class="notice error" role="alert">{lError}</div>{/if}
			{#if linePart}
				<div class="picked">
					<strong>{linePart.name}</strong>
					{#if linePart.brand || linePart.part_number}
						<span class="hint"
							>{linePart.brand ?? ''}{#if linePart.brand && linePart.part_number}{' · '}{/if}<span class="mono"
								>{linePart.part_number ?? ''}</span
							></span
						>
					{/if}
					<button class="btn" type="button" onclick={() => (picking = true)}>{t('parts.pickAnother')}</button>
				</div>
			{:else}
				<p class="hint">{t('deliveries.unmatchedHint')}</p>
				<button class="btn block spaced" type="button" onclick={() => (picking = true)}
					>{t('deliveries.choosePart')}</button
				>
			{/if}
			<label class="field">
				{t('deliveries.description')}
				<input type="text" bind:value={lDesc} maxlength="160" autocomplete="off" />
			</label>
			<label class="field">
				{t('deliveries.article')}
				<input type="text" class="mono" bind:value={lArticle} maxlength="60" autocomplete="off" spellcheck="false" />
			</label>
			<div class="three-cols">
				<label class="field">
					{linePart ? t('parts.quantity', { unit: t(`units.${linePart.unit}`) }) : t('deliveries.quantity')}
					<input
						bind:this={lQtyInput}
						type="text"
						inputmode="decimal"
						bind:value={lQty}
						maxlength="12"
						autocomplete="off"
						onfocus={(e) => e.currentTarget.select()}
						required
					/>
				</label>
				<label class="field">
					{t('deliveries.unitCost', { currency })}
					<input type="text" inputmode="decimal" bind:value={lCost} maxlength="16" autocomplete="off" required />
				</label>
				<label class="field">
					{t('deliveries.vatPercent')}
					<input type="text" inputmode="numeric" bind:value={lVat} maxlength="3" autocomplete="off" required />
				</label>
			</div>
			<button class="btn primary block" type="submit" disabled={lBusy}>
				{lineId ? t('common.saveChanges') : t('deliveries.addLineSubmit')}
			</button>
			{#if lineId}
				<button class="btn danger block gap" type="button" disabled={lBusy} onclick={deleteLine}
					>{t('deliveries.deleteLine')}</button
				>
			{/if}
		</form>
	{/if}
</Sheet>

<Sheet bind:open={postOpen} title={t('deliveries.post')}>
	<form onsubmit={post}>
		{#if postError}<div class="notice error" role="alert">{postError}</div>{/if}
		<p class="hint">{t('deliveries.postHint')}</p>
		{#if d.markup_percent === null}
			<p class="hint">{t('deliveries.noMarkup')} <a href="/account">{t('deliveries.setMarkup')}</a></p>
		{/if}
		<ul class="prices">
			{#each priceRows as r (r.part_id)}
				<li>
					<strong class="pname">{r.name}</strong>
					<span class="hint">
						{t('deliveries.costChange', {
							from: r.previous === null ? '—' : formatMoney(r.previous, currency),
							to: formatMoney(r.cost, currency)
						})}
						{#if r.previous !== null && r.cost > r.previous}<span class="badge danger">{t('deliveries.costUp')}</span>{/if}
					</span>
					<span class="hint">
						{t('deliveries.shelfPrice')}: {r.current === null ? '—' : formatMoney(r.current, currency)}{#if r.suggested !== null}{' · ' +
								t('deliveries.suggested', { price: formatMoney(r.suggested, currency) })}{/if}
					</span>
					<label class="check">
						<input type="checkbox" bind:checked={r.update} />
						{t('deliveries.updatePrice')}
					</label>
					{#if r.update}
						<label class="field">
							{t('deliveries.newPrice', { currency })}
							<input type="text" inputmode="decimal" bind:value={r.price} maxlength="16" autocomplete="off" />
						</label>
					{/if}
				</li>
			{/each}
		</ul>
		<button class="btn primary block" type="submit" disabled={postBusy}>{t('deliveries.postSubmit')}</button>
	</form>
</Sheet>

<style>
	.label {
		display: block;
		font-weight: 500;
		margin-bottom: 6px;
	}
	.doc-types {
		margin-bottom: 6px;
	}
	.crumbs {
		margin: 0 0 6px;
	}
	.head {
		display: flex;
		align-items: center;
		gap: 12px;
		flex-wrap: wrap;
	}
	.head h1 {
		margin: 0;
	}
	.name {
		font-weight: 600;
		overflow-wrap: anywhere;
	}
	.line .row-link {
		cursor: default;
	}
	.line button.row-link {
		cursor: pointer;
	}
	.up {
		color: var(--danger);
	}
	.match {
		display: flex;
		gap: 8px;
		padding: 0 var(--gutter) 12px;
	}
	.match > .btn {
		flex: 1 1 0;
	}
	.add-lines {
		margin: 0 0 16px;
	}
	.totals dl {
		display: grid;
		grid-template-columns: 1fr auto;
		gap: 6px 14px;
		margin: 0 0 8px;
	}
	.totals dd {
		margin: 0;
		text-align: right;
	}
	.files {
		list-style: none;
		padding: 0;
		margin: 0 0 12px;
	}
	.files li {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		border-bottom: 1px solid var(--line);
	}
	.files a {
		display: grid;
		gap: 2px;
		min-height: var(--tap);
		padding: 6px 0;
		align-content: center;
		min-width: 0;
	}
	.fname {
		overflow-wrap: anywhere;
	}
	.remove {
		width: var(--tap);
		height: var(--tap);
		flex: none;
		display: grid;
		place-items: center;
		background: transparent;
		border: none;
		color: var(--danger);
		cursor: pointer;
	}
	.remove svg {
		width: 22px;
		height: 22px;
		stroke: currentColor;
		stroke-width: 2.4;
		stroke-linecap: round;
	}
	.final {
		display: grid;
		gap: 10px;
		margin: 8px 0 24px;
	}
	.final .hint {
		margin: 0;
	}
	.picked {
		display: grid;
		gap: 4px;
		padding: 10px 12px;
		margin-bottom: 14px;
		border: 2px solid var(--accent);
		border-radius: var(--radius);
	}
	.picked .btn {
		justify-self: start;
		margin-top: 4px;
	}
	.spaced {
		margin-bottom: 14px;
	}
	.three-cols {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 0 12px;
	}
	.three-cols > :last-child {
		grid-column: 1 / -1;
	}
	@media (min-width: 480px) {
		.three-cols {
			grid-template-columns: 1fr 1fr 6rem;
		}
		.three-cols > :last-child {
			grid-column: auto;
		}
	}
	.gap {
		margin-top: 10px;
	}
	.prices {
		list-style: none;
		padding: 0;
		margin: 0 0 14px;
	}
	.prices li {
		display: grid;
		gap: 4px;
		padding: 10px 0;
		border-bottom: 1px solid var(--line);
	}
	.pname {
		overflow-wrap: anywhere;
	}
	.check {
		display: flex;
		align-items: center;
		gap: 10px;
		min-height: var(--tap);
		font-weight: 500;
	}
	.check input {
		width: 24px;
		height: 24px;
	}
</style>
