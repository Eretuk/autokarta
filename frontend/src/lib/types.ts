// Shapes returned by the API. Optional price fields are absent for mechanics.

export type DistanceUnit = 'km' | 'mi';
export type VehicleRole = 'customer' | 'fleet' | 'for_sale';
export type WorkOrderStatus = 'open' | 'done' | 'cancelled';

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
	kind: 'labour';
	description: string;
	quantity_milli: number;
	unit_price_minor?: number;
	line_total_minor?: number;
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

export interface BusinessSettings {
	currency: string;
	distance_unit: DistanceUnit;
}
