-- Enable the `uuid-ossp` extension for generating UUIDs if not already enabled
--CREATE EXTENSION IF NOT EXISTS --"uuid-ossp";

-- Table for contract types
CREATE INDEX idx_contract_types_shop_type ON contract_types (shop_type);
CREATE TABLE contract_types (
    id UUID PRIMARY KEY DEFAULT --uuid_generate_v4(),
    shop_type TEXT NOT NULL,
    formula_per_day TEXT NOT NULL,
    max_sum_insured REAL NOT NULL,
    theft_insured BOOLEAN NOT NULL,
    description TEXT,
    conditions TEXT,
    active BOOLEAN NOT NULL,
    min_duration_days INTEGER NOT NULL,
    max_duration_days INTEGER NOT NULL
);

-- Table for items
CREATE TABLE items (
    id SERIAL PRIMARY KEY,
    brand TEXT NOT NULL,
    model TEXT NOT NULL,
    price REAL NOT NULL,
    description TEXT,
    serial_no TEXT NOT NULL
);

-- Table for contracts
CREATE INDEX idx_contracts_uuid ON contracts (id);
CREATE TABLE contracts (
    id UUID PRIMARY KEY DEFAULT --uuid_generate_v4(),
    username TEXT NOT NULL,
    item_id INTEGER REFERENCES items(id),
    start_date TIMESTAMP NOT NULL,
    end_date TIMESTAMP NOT NULL,
    void BOOLEAN NOT NULL,
    contract_type_id UUID REFERENCES contract_types(id),
    claim_index UUID[] -- Array of claim UUIDs
);

-- Table for claims
CREATE INDEX idx_claims_claim_id ON claims (id);
CREATE TABLE claims (
    id UUID PRIMARY KEY DEFAULT --uuid_generate_v4(),
    contract_id UUID REFERENCES contracts(id),
    date TIMESTAMP NOT NULL,
    description TEXT NOT NULL,
    is_theft BOOLEAN NOT NULL,
    status TEXT NOT NULL, -- Should map to the ClaimStatus enum 
    reimbursable REAL NOT NULL,
    repaired BOOLEAN NOT NULL,
    file_reference TEXT
);

-- Table for users
CREATE INDEX idx_users_username ON users (username);
CREATE TABLE users (
    username TEXT PRIMARY KEY,
    password TEXT NOT NULL,
    first_name TEXT NOT NULL,
    last_name TEXT NOT NULL,
    contract_index UUID[] -- Array of contract UUIDs
);

-- Table for repair orders
CREATE TABLE repair_orders (
    id UUID PRIMARY KEY DEFAULT --uuid_generate_v4(),
    claim_id UUID REFERENCES claims(id),
    contract_id UUID REFERENCES contracts(id),
    item_id INTEGER REFERENCES items(id),
    ready BOOLEAN NOT NULL
);
