# SRS: Authentication

## 1. Introduction
The Authentication module manages user identity through secure login, registration, and session management using JWT (JSON Web Tokens).

## 2. Access Control
- **Roles**: Anonymous users (public), Authenticated users (session management).
- **Permissions**: Public access to Login/Register pages.

## 3. UI Components
- **Login Form**: Email, Password, "Remember Me", Social Login (optional).
- **Registration Form**: Username, Email, Password, Password Confirmation.
- **Forgot Password**: Email submission for recovery tokens.

## 4. Functional Requirements

### [SRS-AUT-001] User Login
- **Trigger**: Click "Sign In" on Login screen.
- **Input**: `email`, `password`.
- **Output**: Redirect to Dashboard; Store JWT in `localStorage` or `HTTP-Only` cookie.
- **System Logic**: Calls `POST /api/v1/auth/login`. Validates password hash (Argon2/bcrypt).
- **Validation**:
  - Proper email format.
  - Client-side throttle/retry policy. (Server-side auth rate-limit middleware chưa bật mặc định.)

### [SRS-AUT-002] User Registration
- **Trigger**: Click "Sign Up" on Register screen.
- **Input**: `username`, `email`, `password`.
- **Output**: "Registration Successful" toast; redirect to Login.
- **System Logic**: Calls `POST /api/v1/auth/register`. Creates a new user entry with default `viewer` role.
- **Validation**:
  - Email uniqueness check.
  - Password complexity (min 8 chars, 1 number, 1 symbol).

### [SRS-AUT-003] Logout
- **Trigger**: Click "Logout" in AppShell/Header.
- **Input**: None.
- **Output**: Purge local session data; redirect to Public Home/Login.
- **System Logic**: Calls `POST /api/v1/auth/logout` (to blacklist token if required).

### [SRS-AUT-004] Session Persistence
- **Trigger**: Initial App Load.
- **Input**: Stored token.
- **Output**: Populated `UserStore`; bypass login if token is valid.
- **System Logic**: Parse user ID từ JWT claims, sau đó gọi `GET /api/v1/users/:id`.

### [SRS-AUT-005] Unauthorized Access Handling
- **Trigger**: Accessing protected route or API returns 401.
- **Input**: None.
- **Output**: Redirect to Login with `from` location state.

## 5. Non-Functional Requirements
- **Security**: All passwords must be hashed before storage. No plain-text passwords log entries.
- **Privacy**: No sensitive user data (PII) should be exposed via public APIs.
