# UI Flow: Profile & Administration

These screens handle individual user identity and platform-wide user management.

## 1. My Profile Screen
- **Screen**: `ProfilePage.tsx`
- **Action: Change Avatar**
    - **Trigger**: Click avatar circle.
    - **Process**: 
        1. Calls `POST /api/v1/users/avatar/upload-url` (Get S3 presigned URL).
        2. Uploads file directly to S3.
        3. Stores key in database via `PUT /api/v1/users/:id`.
- **Action: Update Profile**
    - **Trigger**: Click "Save Changes".
    - **Submission**: Calls `PUT /api/v1/users/:id`.
- **Action: Change Password**
    - **Trigger**: Enter passwords and click "Change Password".
    - **Submission**: Calls `PUT /api/v1/users/:id/password`.

## 2. User Management (Admin Dashboard)
- **Screen**: `UserManagementPage.tsx`
- **Overview**: A master table of all platform users with role and agent pairing info.

### Actions
- **Action: Invite User**
    - **Trigger**: Click "Invite User" button.
    - **Submission**: Chưa có endpoint backend riêng cho invite.
- **Action: Edit Roles**
    - **Trigger**: Click "Edit Roles" in the user action menu.
    - **Submission**: Calls `PUT /api/v1/users/:id` với payload `global_roles`.
- **Action: Delete User**
    - **Trigger**: Click "Delete User" (requires confirmation).
    - **Submission**: Calls `DELETE /api/v1/users/:id`.
- **Action: Export CSV**
    - **Trigger**: Click "Export CSV".
    - **Effect**: Generates a standard CSV download of the current filtered user list client-side.
- **Action: Filter by Role/Status**
    - **Trigger**: Change dropdown value.
    - **Effect**: Calls `filterByRole()` / `filterByStatus()` in the `useUsers` hook to refine the table.
