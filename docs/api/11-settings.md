# Settings API

API endpoints cho system settings.

## Base Path

`/api/v1/settings`

## Authentication

Hiện tại handler chưa enforce JWT/Admin role trực tiếp cho endpoints settings.

---

## Endpoints

### 1. GET `/api/v1/settings`

Lấy system settings.

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Settings retrieved successfully",
  "data": {
    "cloudflare_account_id": "...",
    "cloudflare_api_token": "...",
    "encryption_key": "..."
  }
}
```

**Note**: Sensitive fields có thể bị mask trong response.

**Permissions**: Admin only.

#### Frontend Usage

**File**: `frontend/src/api/settings.ts`

**Màn hình**: Settings Page (`/settings`)

**Backend**: `crates/server/src/routes/settings.rs::get_settings`

---

### 2. PUT `/api/v1/settings`

Cập nhật system settings.

#### Request Body

```json
{
  "cloudflare_account_id": "...",
  "cloudflare_api_token": "..."
}
```

**Fields** (all optional):
- `cloudflare_account_id` (string)
- `cloudflare_api_token` (string)
- `encryption_key` (string)

**Permissions**: Admin only.

#### Response

**Status**: `200 OK`

**Body**: Updated settings object

#### Frontend Usage

**File**: `frontend/src/pages/SettingsPage.tsx`

**Màn hình**: Settings Page

**Backend**: `crates/server/src/routes/settings.rs::update_settings`

---

## Settings Fields

- **cloudflare_account_id**: Cloudflare account ID cho deployments
- **cloudflare_api_token**: Cloudflare API token
- **encryption_key**: Encryption key cho sensitive data

## Security Notes

- Settings được encrypt trong database
- API tokens được mask trong responses
- Chỉ admin có thể view và update settings
