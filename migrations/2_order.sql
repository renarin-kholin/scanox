create table "order"
(
    order_id uuid primary key default uuid_generate_v1mc(),
    from_number text not null,
    document_id text not null,
    razorpay_order_id text,
    copies smallint default 1,
    is_both_side boolean default false,
    is_color boolean default false,
    is_paid boolean default false,
    is_received boolean default false,
    created_at timestamptz not null default now(),
    updated_at timestamptz
);
