CREATE TYPE SESSION_STATUS AS ENUM (
    'pending',
    'in_progress',
    'success',
    'failed'
    );

CREATE TYPE REQ_TYPE AS ENUM (
    'send_runes',
    'create_transaction',
    'broadcast_transaction',
    'generate_frost_signature'
    );

CREATE TABLE IF NOT EXISTS gateway.session_requests
(
    session_id   UUID PRIMARY KEY,
    request_type TEXT           NOT NULL,
    status       SESSION_STATUS NOT NULL DEFAULT 'pending',
    request      JSONB          NOT NULL,
    response     JSONB,
    error        TEXT,
    created_at   TIMESTAMP               DEFAULT now(),
    updated_at   TIMESTAMP               DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_session_requests_status ON gateway.session_requests (status);
CREATE INDEX IF NOT EXISTS idx_session_requests_type ON gateway.session_requests (request_type);
CREATE INDEX IF NOT EXISTS idx_session_requests_created_at ON gateway.session_requests (created_at);
CREATE INDEX IF NOT EXISTS idx_session_requests_type_status ON gateway.session_requests (request_type, status);