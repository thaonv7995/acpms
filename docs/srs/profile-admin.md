# SRS: Profile & Administration

## 1. Introduction
This document covers individual user profile management and platform-level administration of users and roles.

## 2. Access Control
- **Roles**:
  - `All Users`: Can access their own profile.
  - `Admins`: Can access the User Management dashboard.

## 3. UI Components
- **Profile Card**: Avatar upload, Display Name, Account Email (Read-only).
- **Security Section**: Password reset form.
- **Admin Table**: Searchable/Filterable list of all system users.
- **User Actions Menu**: Edit Roles, Revoke Tokens, Delete User.

## 4. Functional Requirements

### [SRS-ADM-001] Manage Own Profile
- **Trigger**: Click "Save" on Profile screen.
- **Input**: `name`, `avatar_file`.
- **Output**: Instant UI update; Header avatar updates.
- **System Logic**: Calls `PUT /api/v1/users/:id`.
- **Validation**: Avatar file size < 2MB.

### [SRS-ADM-002] Multi-Role Assignment (Admin Only)
- **Trigger**: Click "Edit Roles" in Admin table.
- **Input**: Selection of roles (Admin, Dev, PO, etc.).
- **Output**: Updated user row status.
- **System Logic**: Calls `PUT /api/v1/users/:id` với trường `global_roles`. Server kiểm tra role admin trước khi cho phép đổi roles.

### [SRS-ADM-003] Invite User (Admin Only)
- **Trigger**: "Invite User" button in Admin dashboard.
- **Input**: `email`, `initial_roles`.
- **Output**: Invitation email sent (placeholder); user appears in "Pending" status.
- **System Logic**: Chưa có endpoint invite riêng trong backend hiện tại.

### [SRS-ADM-004] Revoke User Access (Admin Only)
- **Trigger**: "Delete User" or "Deactivate" action.
- **Input**: `user_id`.
- **Output**: User session invalidated; user removed from active list.
- **System Logic**: Calls `DELETE /api/v1/users/:id`.

### [SRS-ADM-005] Export User Data
- **Trigger**: "Export CSV" button.
- **Input**: Current filter state.
- **Output**: Binary CSV file download.
- **System Logic**: Client-side generation from current `users` buffer.

## 5. Non-Functional Requirements
- **Safety**: Admins cannot delete their own account from the Admin screen.
- **Privacy**: The Admin screen must obscure sensitive data (like full IP logs) unless specifically requested by an "Audit Mode".
