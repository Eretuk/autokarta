// Shapes returned by the API. Optional price fields are absent for mechanics.

export type DistanceUnit = 'km' | 'mi';
export type VehicleRole = 'customer' | 'fleet' | 'for_sale';
export type WorkOrderStatus = 'open' | 'done' | 'cancelled';
export type PartUnit = 'pcs' | 'l' | 'kg' | 'm';
export type PartCondition = 'new' | 'used';
export type MovementKind = 'receipt' | 'consumption' | 'adjustment';

export interface Client {
	id: string;
	name: string;
	phone: string | null;
	note: string | null;
}

export interface VehicleSummary {
	id: string;
	plate: string | null;
	vin: string | null;
	frame_number: string | null;
	make: string | null;
	model: string | null;
	year: number | null;
	role: VehicleRole;
	odometer_unit: DistanceUnit;
	owner_name: string | null;
	last_odometer: number | null;
	open_work_orders: number;
}

export interface Ownership {
	id: string;
	client_id: string;
	name: string;
	phone: string | null;
	started_on: string;
	ended_on: string | null;
}

export interface Reading {
	id: string;
	reading: number;
	read_on: string;
	work_order_id: string | null;
	work_order_number: number | null;
}

export interface VehicleWorkOrder {
	id: string;
	number: number;
	status: WorkOrderStatus;
	opened_on: string;
	complaint: string | null;
	odometer: number | null;
	item_count: number;
	total_minor?: number;
}

export interface VehicleDetail {
	id: string;
	vin: string | null;
	frame_number: string | null;
	plate: string | null;
	make: string | null;
	model: string | null;
	year: number | null;
	engine: string | null;
	note: string | null;
	role: VehicleRole;
	odometer_unit: DistanceUnit;
	created_at: string;
	owner: Client | null;
	owners: Ownership[];
	odometer: Reading[];
	work_orders: VehicleWorkOrder[];
}

export interface WorkOrderItem {
	id: string;
	kind: 'labour' | 'part';
	description: string;
	quantity_milli: number;
	part_id?: string;
	part_number?: string;
	brand?: string;
	condition?: PartCondition;
	unit?: PartUnit;
	unit_price_minor?: number;
	line_total_minor?: number;
	/** Owners only: net purchase cost of the pieces it took (FIFO). */
	cost_minor?: number | null;
}

export interface WorkOrderDetail {
	id: string;
	number: number;
	status: WorkOrderStatus;
	opened_on: string;
	complaint: string | null;
	notes: string | null;
	created_by_name: string | null;
	vehicle: {
		id: string;
		plate: string | null;
		vin: string | null;
		frame_number: string | null;
		make: string | null;
		model: string | null;
		year: number | null;
		odometer_unit: DistanceUnit;
	};
	client: Client | null;
	odometer: number | null;
	items: WorkOrderItem[];
	totals?: { currency: string; total_minor: number; unpriced_items: number };
}

export interface WorkOrderListRow {
	id: string;
	number: number;
	status: WorkOrderStatus;
	opened_on: string;
	complaint: string | null;
	vehicle_id: string;
	plate: string | null;
	vin: string | null;
	frame_number: string | null;
	make: string | null;
	model: string | null;
	client_name: string | null;
	item_count: number;
}

export interface Organization {
	company_name: string | null;
	tax_id: string | null;
	regon: string | null;
	legal_address: string | null;
	service_address: string | null;
	phone: string | null;
	email: string | null;
	contact_person: string | null;
	bank_account: string | null;
}

export interface BusinessSettings {
	currency: string;
	distance_unit: DistanceUnit;
	default_vat_percent: number;
	/** Owners only: default markup on the net cost; null: no suggested prices. */
	markup_percent?: number | null;
	organization: Organization;
}

export interface PartSummary {
	id: string;
	name: string;
	brand: string | null;
	part_number: string | null;
	condition: PartCondition;
	unit: PartUnit;
	location: string | null;
	stock_milli: number;
	min_stock_milli: number;
	sell_price_minor?: number | null;
	/** The search text is exactly one of this part's barcodes. */
	barcode_match: boolean;
}

export interface StockMovement {
	id: string;
	kind: MovementKind;
	quantity_milli: number;
	unit_cost_minor?: number;
	note: string | null;
	created_at: string;
	created_by_name: string | null;
	work_order_id: string | null;
	work_order_number: number | null;
	delivery_id: string | null;
	delivery_number: number | null;
}

export interface PartDetail {
	id: string;
	name: string;
	brand: string | null;
	part_number: string | null;
	condition: PartCondition;
	unit: PartUnit;
	location: string | null;
	min_stock_milli: number;
	sell_price_minor?: number | null;
	note: string | null;
	stock_milli: number;
	/** Owners only: latest net purchase cost. */
	last_cost_minor?: number | null;
	last_cost_vat_percent?: number | null;
	barcodes: string[];
	movements: StockMovement[];
	/** Owners only: pieces on the shelf by purchase, next to go out first. */
	fifo_layers?: { quantity_milli: number; unit_cost_minor: number }[];
}

/** GET /version. `latest` only for owners, once the server checked GitHub. */
export interface VersionInfo {
	version: string;
	latest?: { version: string; url: string; published_at: string | null };
	update_available: boolean;
}

export interface Supplier {
	id: string;
	name: string;
	tax_id: string | null;
	note: string | null;
}

export type DeliveryStatus = 'draft' | 'posted';

/** invoice: faktura; receipt: shop receipt (paragon); none: no invoice,
 * e.g. a used part bought from a private person. */
export type DocumentType = 'invoice' | 'receipt' | 'none';

export interface DeliveryListRow {
	id: string;
	number: number;
	status: DeliveryStatus;
	document_type: DocumentType;
	supplier_id: string | null;
	supplier_name: string | null;
	invoice_number: string | null;
	invoice_date: string;
	item_count: number;
	file_count: number;
	total_net_minor: number;
	total_gross_minor: number;
}

export interface DeliveryItem {
	id: string;
	part_id: string | null;
	/** What the invoice calls it. */
	description: string | null;
	/** The supplier's article as on the invoice. */
	part_number: string | null;
	quantity_milli: number;
	unit_cost_minor: number;
	vat_percent: number;
	part_name: string | null;
	part_brand: string | null;
	part_part_number: string | null;
	part_condition: PartCondition | null;
	part_unit: PartUnit | null;
	part_sell_price_minor: number | null;
	previous_cost_minor: number | null;
	total_net_minor: number;
	suggested_price_minor?: number;
}

export interface DeliveryFile {
	id: string;
	file_name: string;
	content_type: string;
	size_bytes: number;
	created_at: string;
}

export interface DeliveryDetail {
	id: string;
	number: number;
	status: DeliveryStatus;
	document_type: DocumentType;
	supplier_id: string | null;
	supplier_name: string | null;
	supplier_tax_id: string | null;
	invoice_number: string | null;
	invoice_date: string;
	invoice_total_net_minor: number | null;
	note: string | null;
	posted_at: string | null;
	posted_by_name: string | null;
	items: DeliveryItem[];
	files: DeliveryFile[];
	totals: { net_minor: number; vat_minor: number; gross_minor: number };
	currency: string;
	markup_percent: number | null;
	default_vat_percent: number;
}

export interface StockValue {
	value_minor: number;
	unvalued_parts: number;
	negative_parts: number;
}

export interface MonthRow {
	/** First day of the month, YYYY-MM-DD. */
	month: string;
	purchased_net_minor: number;
	purchased_lines: number;
	purchased_unpriced: number;
	used_cost_minor: number;
	used_sale_minor: number;
	used_lines: number;
	used_unpriced: number;
	used_unvalued: number;
	stock_end: StockValue;
}

export interface MonthsReport {
	year: number;
	currency: string;
	months: MonthRow[];
}

export interface StockReportRow {
	id: string;
	name: string;
	brand: string | null;
	part_number: string | null;
	condition: PartCondition;
	unit: PartUnit;
	location: string | null;
	quantity_milli: number;
	unit_cost_minor: number | null;
	value_minor: number | null;
}

export interface StockReport {
	date: string;
	currency: string;
	total: StockValue;
	rows: StockReportRow[];
}
