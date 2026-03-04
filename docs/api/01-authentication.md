# Authentication API

API endpoints cho authentication, đăng ký, đăng nhập, và quản lý tokens.

## Base Path

`/api/v1/auth`

## Authentication Method

`POST /auth/register`, `POST /auth/login`, `POST /auth/refresh` là public.  
`POST /auth/logout` và `POST /auth/revoke/:user_id` yêu cầu JWT Bearer token.

---

## Endpoints

### 1. POST `/api/v1/auth/register`

Đăng ký user mới.

#### Request

**Headers**:
```
Content-Type: application/json
```

**Body**:
```json
{
  "email": "user@example.com",
  "name": "User Name",
  "password": "password123"
}
```

**Fields**:
- `email` (string, required): Email address, phải là valid email format
- `name` (string, required): Tên user, 1-100 characters
- `password` (string, required): Mật khẩu, tối thiểu 8 characters
- System tự gán `global_roles = ["viewer"]` khi self-register (không cho client truyền role)

**Validation Rules**:
- Email: Must be valid email format
- Name: 1-100 characters
- Password: Minimum 8 characters, must contain at least one letter and one number

#### Response

**Status**: `201 Created`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "User registered successfully",
  "data": {
    "access_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
    "refresh_token": "abc123-def456-ghi789...",
    "expires_in": 1800,
    "user": {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "email": "user@example.com",
      "name": "User Name",
      "avatar_url": null,
      "gitlab_username": null,
      "global_roles": ["viewer"],
      "created_at": "2026-01-13T10:00:00Z"
    }
  },
  "metadata": null,
  "error": null
}
```

**Fields**:
- `access_token` (string): JWT access token, expires in 30 minutes
- `refresh_token` (string): Refresh token, expires in 7 days
- `expires_in` (number): Seconds until access token expires (1800 = 30 minutes)
- `user` (object): User information

#### Error Responses

**409 Conflict** - Email already exists:
```json
{
  "success": false,
  "code": "4091",
  "message": "Email already exists",
  "data": null,
  "error": {
    "details": "A user with this email already exists",
    "trace_id": "abc123-def456"
  }
}
```

**400 Bad Request** - Validation error:
```json
{
  "success": false,
  "code": "4001",
  "message": "Validation error",
  "data": null,
  "error": {
    "details": "Field 'email' is required",
    "trace_id": "abc123-def456"
  }
}
```

#### Frontend Usage

**File**: `frontend/src/pages/RegisterPage.tsx`

**Example**:
```typescript
import { apiPost } from '@/api/client';

const handleRegister = async (formData: {
  email: string;
  name: string;
  password: string;
}) => {
  try {
    const response = await apiPost<AuthResponse>('/auth/register', {
      email: formData.email,
      name: formData.name,
      password: formData.password
    });
    
    // Store tokens
    localStorage.setItem('acpms_token', response.access_token);
    localStorage.setItem('acpms_refresh_token', response.refresh_token);
    
    // Redirect to dashboard
    navigate('/dashboard');
  } catch (error) {
    // Handle error
    console.error('Registration failed:', error);
  }
};
```

**Màn hình**: Register Page (`/register`)

**Nhiệm vụ**: 
- Hiển thị form đăng ký
- Validate input
- Gọi API và lưu tokens
- Redirect đến dashboard sau khi đăng ký thành công

#### Backend Implementation

**File**: `crates/server/src/routes/auth.rs::register`

**Logic**:
1. Validate request body
2. Validate password strength
3. Hash password với bcrypt
4. Insert user vào database
5. Generate JWT access token
6. Generate refresh token và lưu vào database
7. Return tokens và user info

**Database Tables**:
- `users`: Lưu user information
- `refresh_tokens`: Lưu refresh tokens

---

### 2. POST `/api/v1/auth/login`

Đăng nhập user.

#### Request

**Headers**:
```
Content-Type: application/json
```

**Body**:
```json
{
  "email": "user@example.com",
  "password": "password123"
}
```

**Fields**:
- `email` (string, required): Email address
- `password` (string, required): Mật khẩu

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Login successful",
  "data": {
    "access_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
    "refresh_token": "abc123-def456-ghi789...",
    "expires_in": 1800,
    "user": {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "email": "user@example.com",
      "name": "User Name",
      "avatar_url": "https://...",
      "gitlab_username": "username",
      "global_roles": ["viewer"],
      "created_at": "2026-01-13T10:00:00Z"
    }
  },
  "metadata": null,
  "error": null
}
```

#### Error Responses

**401 Unauthorized** - Invalid credentials:
```json
{
  "success": false,
  "code": "4011",
  "message": "Invalid credentials",
  "data": null,
  "error": {
    "details": "Email or password is incorrect",
    "trace_id": "abc123-def456"
  }
}
```

#### Frontend Usage

**File**: `frontend/src/pages/LoginPage.tsx`

**Example**:
```typescript
import { apiPost } from '@/api/client';

const handleLogin = async (email: string, password: string) => {
  try {
    const response = await apiPost<AuthResponse>('/auth/login', {
      email,
      password
    });
    
    // Store tokens
    localStorage.setItem('acpms_token', response.access_token);
    localStorage.setItem('acpms_refresh_token', response.refresh_token);
    
    // Redirect to dashboard
    navigate('/dashboard');
  } catch (error) {
    // Handle error
    if (error.code === '4011') {
      setError('Invalid email or password');
    }
  }
};
```

**Màn hình**: Login Page (`/login`)

**Nhiệm vụ**: 
- Hiển thị form đăng nhập
- Validate input
- Gọi API và lưu tokens
- Redirect đến dashboard sau khi đăng nhập thành công

#### Backend Implementation

**File**: `crates/server/src/routes/auth.rs::login`

**Logic**:
1. Fetch user by email
2. Verify password với bcrypt
3. Generate JWT access token
4. Generate refresh token và lưu vào database
5. Return tokens và user info

---

### 3. POST `/api/v1/auth/refresh`

Refresh access token khi access token hết hạn.

#### Request

**Headers**:
```
Content-Type: application/json
```

**Body**:
```json
{
  "refresh_token": "abc123-def456-ghi789..."
}
```

**Fields**:
- `refresh_token` (string, required): Refresh token từ lần login/register trước

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Token refreshed successfully",
  "data": {
    "access_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
    "expires_in": 1800
  },
  "metadata": null,
  "error": null
}
```

**Note**: Implementation hiện tại không rotate refresh token trong endpoint này.

#### Error Responses

**401 Unauthorized** - Invalid refresh token:
```json
{
  "success": false,
  "code": "4013",
  "message": "Invalid refresh token",
  "data": null,
  "error": {
    "details": "Refresh token is invalid or expired",
    "trace_id": "abc123-def456"
  }
}
```

#### Frontend Usage

**File**: `frontend/src/api/client.ts`

**Example** (Auto refresh trong API client):
```typescript
async function authenticatedFetch(path: string, options: RequestInit = {}) {
  const token = localStorage.getItem('acpms_token');
  
  const response = await fetch(`${API_BASE_URL}${path}`, {
    ...options,
    headers: {
      ...options.headers,
      'Authorization': `Bearer ${token}`,
      'Content-Type': 'application/json',
    },
  });
  
  // Auto refresh token on 401
  if (response.status === 401) {
    const refreshToken = localStorage.getItem('acpms_refresh_token');
    if (refreshToken) {
      try {
        const refreshResponse = await apiPost<{ access_token: string; expires_in: number }>('/auth/refresh', {
          refresh_token: refreshToken
        });
        
        localStorage.setItem('acpms_token', refreshResponse.access_token);
        
        // Retry original request
        return authenticatedFetch(path, options);
      } catch (error) {
        // Refresh failed, redirect to login
        localStorage.removeItem('acpms_token');
        localStorage.removeItem('acpms_refresh_token');
        window.location.href = '/login';
        throw error;
      }
    }
  }
  
  return response;
}
```

**Nhiệm vụ**: 
- Tự động refresh token khi access token hết hạn
- Retry request sau khi refresh thành công
- Redirect đến login nếu refresh token cũng hết hạn

#### Backend Implementation

**File**: `crates/server/src/routes/auth.rs::refresh_token`

**Logic**:
1. Verify refresh token
2. Check refresh token trong database và chưa expired
3. Generate JWT access token mới
4. Return `access_token` + `expires_in`

---

### 4. POST `/api/v1/auth/logout`

Đăng xuất user và invalidate tokens.

#### Request

**Headers**:
```
Authorization: Bearer <access_token>
```

**Body**:
```json
{
  "refresh_token": "abc123-def456-ghi789..."
}
```

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Logged out successfully",
  "data": null,
  "metadata": null,
  "error": null
}
```

**Note**: Refresh token sẽ bị invalidate trong database.

#### Frontend Usage

**File**: `frontend/src/api/auth.ts`

**Example**:
```typescript
import { apiPost } from './client';

export async function logout(): Promise<void> {
  try {
    const refreshToken = localStorage.getItem('acpms_refresh_token');
    await apiPost('/auth/logout', {
      refresh_token: refreshToken ?? ''
    });
  } catch (error) {
    // Ignore errors, still clear local storage
  } finally {
    // Always clear tokens
    localStorage.removeItem('acpms_token');
    localStorage.removeItem('acpms_refresh_token');
    window.location.href = '/login';
  }
}
```

**Màn hình**: Tất cả các màn hình có logout button

**Nhiệm vụ**: 
- Gọi API để invalidate tokens trên server
- Xóa tokens khỏi localStorage
- Redirect đến login page

#### Backend Implementation

**File**: `crates/server/src/routes/auth.rs::logout`

**Logic**:
1. Extract JWT token từ header
2. Extract JTI (JWT ID) từ token claims
3. Add JTI vào token blacklist
4. Revoke refresh token gửi trong request body
5. Return success

**Database Tables**:
- `token_blacklist`: Lưu blacklisted JWT IDs
- `refresh_tokens`: Xóa refresh token của user

---

### 5. POST `/api/v1/auth/revoke/:user_id`

Revoke tất cả refresh tokens của một user.

#### Request

**Headers**:
```
Authorization: Bearer <access_token>
```

**Path Parameters**:
- `user_id` (UUID, required): ID của user cần revoke tokens

**Body**: Không có

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Tokens revoked successfully",
  "data": null,
  "metadata": null,
  "error": null
}
```

#### Error Responses

**Implementation Note**: route này hiện chưa enforce admin role trong handler.

#### Frontend Usage

**File**: `frontend/src/pages/UserManagementPage.tsx`

**Example**:
```typescript
import { apiPost } from '@/api/client';

const handleRevokeTokens = async (userId: string) => {
  try {
    await apiPost(`/auth/revoke/${userId}`, {});
    // Show success message
    toast.success('Tokens revoked successfully');
  } catch (error) {
    // Handle error
    toast.error('Failed to revoke tokens');
  }
};
```

**Màn hình**: User Management Page (Admin only)

**Nhiệm vụ**: 
- Admin revoke tokens của user khác
- Force user phải đăng nhập lại

#### Backend Implementation

**File**: `crates/server/src/routes/auth.rs::revoke_user_tokens`

**Logic**:
1. Revoke tất cả refresh tokens của `user_id`
2. Return số lượng token đã revoke trong message

---

## Token Structure

### JWT Access Token

**Algorithm**: HS256

**Payload**:
```json
{
  "sub": "550e8400-e29b-41d4-a716-446655440000", // User ID
  "jti": "jwt-id-123", // JWT ID for blacklisting
  "exp": 1705152000, // Expiration timestamp
  "iat": 1705065600 // Issued at timestamp
}
```

**Expiration**: 30 minutes (1800 seconds)

### Refresh Token

**Format**: Random UUID string

**Storage**: Database table `refresh_tokens`

**Expiration**: 7 days

**Fields**:
- `id`: Token ID
- `user_id`: User ID
- `token`: Token string
- `expires_at`: Expiration timestamp
- `user_agent`: User agent string
- `ip_address`: IP address
- `created_at`: Created timestamp

---

## Security Notes

1. **Password Hashing**: Passwords được hash với bcrypt (cost factor 12)
2. **Token Blacklist**: JWT tokens có thể bị blacklist qua JTI
3. **Refresh Flow**: Endpoint `/auth/refresh` hiện trả về access token mới, không rotate refresh token
4. **Rate Limiting**: Middleware rate-limit có trong codebase nhưng chưa gắn vào router mặc định
5. **HTTPS Required**: Production phải sử dụng HTTPS

---

## Frontend Integration

### Token Storage

```typescript
// Store tokens after login/register
localStorage.setItem('acpms_token', accessToken);
localStorage.setItem('acpms_refresh_token', refreshToken);

// Get tokens
const token = localStorage.getItem('acpms_token');
const refreshToken = localStorage.getItem('acpms_refresh_token');

// Clear tokens on logout
localStorage.removeItem('acpms_token');
localStorage.removeItem('acpms_refresh_token');
```

### Auto Token Refresh

API client tự động refresh token khi nhận 401 response. Xem `frontend/src/api/client.ts` để biết implementation chi tiết.
