# Scanox Backend

Backend service for Scanox - a WhatsApp-based document printing service with payment integration.

> **⚠️ Disclaimer**: This project is currently in development and not production-ready. Several advertised features are incomplete or not fully implemented. Use at your own risk in non-production environments.

## Features

- WhatsApp integration for document upload and order management
- Razorpay payment processing
- QR code generation for order verification
- PDF document handling
- Encrypted order verification system

## Setup

1. Configure environment variables:

```bash
cp .env.example .env
# Edit .env with your credentials
```

Required environment variables:

- `DATABASE_URL` - PostgreSQL connection string
- `WHATSAPP_TOKEN` - WhatsApp Business API token
- `RAZORPAY_KEY_ID` / `RAZORPAY_KEY_SECRET` - Razorpay credentials
- `AES_KEY` - Base64-encoded AES-256-GCM key
- `WEBHOOK_SECRET` - Webhook verification token
- `CLIENT_SECRET` - API client authentication
- `PORT` - Server port

2. Run database migrations:

```bash
sqlx migrate run
```

3. Run the development server:

```bash
cargo run
```

## Technology Stack

- Rust
- Axum (web framework)
- SQLx (PostgreSQL)
- Tokio (async runtime)

## API Endpoints

### WhatsApp Webhook

- **GET /webhook/whatsapp** - Webhook verification endpoint for WhatsApp
  - Query params: `hub.mode`, `hub.verify_token`, `hub.challenge`
  - Used by WhatsApp to verify webhook setup
- **POST /webhook/whatsapp** - Receives WhatsApp messages and events
  - Handles document uploads (PDF files)
  - Processes text messages for order workflow
  - Manages order state (copies, print sides, payment)

### Payment Webhook

- **POST /webhook/razorpay** - Razorpay payment webhook
  - Receives payment confirmation events
  - Generates encrypted QR codes for verified orders
  - Sends QR code to customer via WhatsApp

### Order Management (Requires Bearer Token Authentication)

- **POST /verify_qr** - Verifies order QR code

  - Body: `{ "qrcode_data": "encrypted_qr_string" }`
  - Returns: Order details if valid and not expired
  - Used by print shop to validate orders

- **GET /collect_order/{order_id}** - Downloads order document
  - Path param: `order_id` (UUID)
  - Returns: PDF file for printing
  - Marks order as received upon successful download
  - Requires `Authorization: Bearer {CLIENT_SECRET}` header

## Development

```bash
cargo run          # Start development server
cargo test         # Run tests
cargo build --release  # Build for production
```

## Docker

```bash
docker build -t scanox-backend .
docker run -p 8000:8000 scanox-backend
```

## License

MIT License
