create type order_progress as enum ('STARTED', 'COPIES', 'SIDE', 'COLOR', 'PAYMENT','READY', 'DONE');
alter table "order" add column progress order_progress default 'STARTED' not null;