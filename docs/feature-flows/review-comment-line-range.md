# Đề xuất: Comment trên nhiều dòng (Line Range) trong Diff Viewer

## Tổng quan

Hiện tại review comment chỉ hỗ trợ comment trên **1 dòng** (`line_number`). Đề xuất này mở rộng để hỗ trợ **comment trên nhiều dòng** (range), tương tự GitHub/GitLab: user có thể chọn nhiều dòng liên tiếp và gắn 1 comment cho cả đoạn đó.

---

## 1. Database Schema

### 1.1 Migration mới

**File:** `crates/db/migrations/YYYYMMDDHHMMSS_review_comments_line_range.sql`

```sql
-- Add line_number_end for multi-line comment support
-- When line_number_end = NULL or = line_number: single-line comment (backward compatible)
-- When line_number_end > line_number: multi-line comment (lines line_number..line_number_end inclusive)

ALTER TABLE review_comments
ADD COLUMN IF NOT EXISTS line_number_end INTEGER;

COMMENT ON COLUMN review_comments.line_number_end IS 'End line for range comments. NULL or equal to line_number = single-line comment. Must be >= line_number when set.';

-- Update composite index for range queries (optional, for performance)
DROP INDEX IF EXISTS idx_review_comments_file_line;
CREATE INDEX IF NOT EXISTS idx_review_comments_file_line
ON review_comments(attempt_id, file_path, line_number, line_number_end)
WHERE file_path IS NOT NULL;
```

**Lưu ý:**
- `line_number_end` nullable → backward compatible với comment cũ (chỉ có `line_number`)
- Single-line: `line_number_end IS NULL` hoặc `line_number_end = line_number`
- Multi-line: `line_number_end > line_number`, range = `[line_number, line_number_end]` inclusive

---

## 2. Backend (Rust)

### 2.1 Models (`crates/db/src/models.rs`)

```rust
// ReviewComment - thêm field
pub struct ReviewComment {
    // ... existing fields ...
    pub line_number: Option<i32>,
    pub line_number_end: Option<i32>,  // NEW: end line for range, NULL = single-line
    // ...
}

// AddReviewCommentRequest - thêm field
pub struct AddReviewCommentRequest {
    pub file_path: Option<String>,
    pub line_number: Option<i32>,
    pub line_number_end: Option<i32>,  // NEW: optional, must be >= line_number when set
    pub content: String,
}
```

### 2.2 Review Service (`crates/services/src/review-service.rs`)

- **add_comment**: Bind `req.line_number_end` vào INSERT
- **get_comments / get_comment**: SELECT thêm `line_number_end`
- **format_comments_as_feedback**: Cập nhật logic hiển thị location:
  - Single-line: `path:42`
  - Multi-line: `path:42-48`

```rust
let location = match (&comment.file_path, comment.line_number, comment.line_number_end) {
    (Some(path), Some(start), end) => {
        if end.map(|e| e > start).unwrap_or(false) {
            format!("{}:{}-{}", path, start, end.unwrap())
        } else {
            format!("{}:{}", path, start)
        }
    }
    (Some(path), None, _) => format!("{} (file-level)", path),
    (None, _, _) => "General".to_string(),
};
```

### 2.3 API Routes & DTOs

- **ReviewCommentDto** (`crates/server/src/api/dtos.rs`): thêm `line_number_end: Option<i32>`
- **ReviewCommentWithUsersRow** (`crates/server/src/routes/reviews.rs`): thêm `line_number_end`, cập nhật SELECT
- Request body `AddReviewCommentRequest` đã có `line_number_end` từ models

---

## 3. Frontend

### 3.1 Types (`frontend/src/components/review/types.ts`)

```typescript
export interface ReviewComment {
  // ... existing ...
  line_number?: number | null;
  line_number_end?: number | null;  // NEW
  // ...
}

export interface AddCommentRequest {
  attempt_id: string;
  content: string;
  file_path?: string;
  line_number?: number;
  line_number_end?: number;  // NEW: optional, >= line_number
}

export interface LineCommentPosition {
  filePath: string;
  lineNumber: number;
  lineNumberEnd?: number;  // NEW: for range selection
  lineType: 'add' | 'delete' | 'normal';
  x: number;
  y: number;
}
```

### 3.2 InlineCommentForm (`frontend/src/components/diff-viewer/InlineCommentForm.tsx`)

**Props mới:**

```typescript
interface InlineCommentFormProps {
  filePath: string;
  lineNumber: number;
  lineNumberEnd?: number;  // NEW: khi có = multi-line
  onSubmit: (content: string) => Promise<void>;
  onClose: () => void;
}
```

**Label hiển thị:**

```tsx
<span>
  Comment on {lineNumberEnd && lineNumberEnd > lineNumber
    ? `lines ${lineNumber}-${lineNumberEnd}`
    : `line ${lineNumber}`}
</span>
```

### 3.3 UI chọn range (SideBySideView & UnifiedView)

**Flow tương tự GitHub:**

1. **Click dòng đầu** → bắt đầu selection, highlight dòng đó
2. **Shift+Click dòng cuối** (hoặc kéo xuống) → mở rộng selection
3. **Click "Add comment"** trên vùng đã chọn → mở `InlineCommentForm` với `lineNumber` = dòng đầu, `lineNumberEnd` = dòng cuối

**State mới:**

```typescript
// Thay vì: commentLine: { lineNum: number; side: 'left'|'right' } | null
// Dùng: commentRange: { startLine: number; endLine: number; side: 'left'|'right' } | null
```

**Cách implement range selection:**

- **Option A (đơn giản):** Click dòng 1 → set `startLine`. Click dòng 2 (cùng file) → nếu `startLine` đã có, set `endLine = min(start, current)` và `startLine = max(...)` để luôn start <= end. Mở form.
- **Option B (giống GitHub):** Click dòng 1 → set anchor. Shift+click dòng 2 → set range. Hiển thị nút "Add comment" trên vùng highlight.

**DiffLine / UnifiedDiffLine:**

- Thêm prop `isSelected?: boolean` và `isRangeStart?: boolean`, `isRangeEnd?: boolean` để highlight
- Khi click: nếu đang có selection và shift+click → mở rộng range; nếu không → bắt đầu selection mới

### 3.4 Cập nhật handlers

**SideBySideView.tsx:**

```typescript
const [commentRange, setCommentRange] = useState<{
  startLine: number;
  endLine: number;
  side: 'left' | 'right';
} | null>(null);

const handleLineClick = (
  lineNum: number | undefined,
  side: 'left' | 'right',
  isShiftKey: boolean
) => {
  if (!lineNum || !onAddComment) return;
  if (commentRange && isShiftKey && commentRange.side === side) {
    const start = Math.min(commentRange.startLine, lineNum);
    const end = Math.max(commentRange.startLine, lineNum);
    setCommentRange({ startLine: start, endLine: end, side });
  } else {
    setCommentRange({ startLine: lineNum, endLine: lineNum, side });
  }
};

// Khi submit: gửi line_number = startLine, line_number_end = endLine (nếu endLine > startLine)
```

**UnifiedView.tsx:** tương tự, không có `side`.

### 3.5 useReviewComments & getLineComments

**getLineComments** cần mở rộng để match comment với range:

```typescript
// Comment áp dụng cho line nếu:
// - Single-line: c.line_number === lineNumber
// - Multi-line: lineNumber >= c.line_number && lineNumber <= (c.line_number_end ?? c.line_number)
const getLineComments = (filePath: string, lineNumber: number): ReviewComment[] => {
  return (commentsQuery.data ?? []).filter((c) => {
    if (c.file_path !== filePath) return false;
    const start = c.line_number ?? 0;
    const end = c.line_number_end ?? start;
    return lineNumber >= start && lineNumber <= end;
  });
};
```

**addComment** gửi thêm `line_number_end` khi có:

```typescript
// useReviewComments.ts - addComment
async function addComment(request: AddCommentRequest): Promise<ReviewComment> {
  return apiPost<ReviewComment>(...,
    {
      content: request.content,
      file_path: request.file_path,
      line_number: request.line_number,
      line_number_end: request.line_number_end,  // NEW
    }
  );
}
```

### 3.6 CommentItem & LineCommentPopover

**CommentItem.tsx:** hiển thị range khi có:

```tsx
{comment.line_number && (
  <span className="font-mono text-primary">
    :L{comment.line_number}
    {comment.line_number_end != null && comment.line_number_end > comment.line_number
      ? `-${comment.line_number_end}`
      : ''}
  </span>
)}
```

**LineCommentPopover:** nhận `position.lineNumberEnd` và hiển thị "Lines 4-8" trong header.

---

## 4. UX Flow tóm tắt

| Bước | Hành động |
|------|-----------|
| 1 | User click vào dòng 4 trong diff |
| 2 | Dòng 4 được highlight |
| 3a | User click "Add comment" ngay → comment 1 dòng (line 4) |
| 3b | User Shift+click dòng 8 → range 4-8 được highlight |
| 4 | User click "Add comment" trên vùng range → form hiện "Comment on lines 4-8" |
| 5 | User gõ comment, submit → API nhận `line_number: 4, line_number_end: 8` |

---

## 5. Backward Compatibility

- Comment cũ (chỉ có `line_number`): `line_number_end = NULL` → hiển thị như single-line
- API cũ gửi không có `line_number_end` → xử lý như single-line
- Index cũ có thể giữ hoặc cập nhật tùy nhu cầu query

---

## 6. Thứ tự triển khai đề xuất

1. **DB migration** – thêm `line_number_end`
2. **Backend** – models, service, routes, DTOs
3. **Frontend types** – ReviewComment, AddCommentRequest, LineCommentPosition
4. **InlineCommentForm** – props `lineNumberEnd`, label
5. **SideBySideView / UnifiedView** – state range, handleLineClick với Shift
6. **useReviewComments** – addComment gửi `line_number_end`, getLineComments match range
7. **CommentItem / LineCommentPopover** – hiển thị range
8. **DiffLine** – highlight vùng được chọn (optional, UX tốt hơn)

---

## 7. Files cần sửa (checklist)

| Layer | File | Thay đổi |
|-------|------|----------|
| DB | `migrations/..._review_comments_line_range.sql` | ADD COLUMN line_number_end |
| DB | `crates/db/src/models.rs` | ReviewComment, AddReviewCommentRequest |
| Services | `crates/services/src/review-service.rs` | add_comment, get_*, format_* |
| Server | `crates/server/src/api/dtos.rs` | ReviewCommentDto |
| Server | `crates/server/src/routes/reviews.rs` | ReviewCommentWithUsersRow, SELECT |
| Frontend | `frontend/src/components/review/types.ts` | ReviewComment, AddCommentRequest, LineCommentPosition |
| Frontend | `frontend/src/components/diff-viewer/InlineCommentForm.tsx` | lineNumberEnd prop, label |
| Frontend | `frontend/src/components/diff-viewer/SideBySideView.tsx` | commentRange state, handleLineClick |
| Frontend | `frontend/src/components/diff-viewer/UnifiedView.tsx` | tương tự |
| Frontend | `frontend/src/components/review/useReviewComments.ts` | addComment, getLineComments |
| Frontend | `frontend/src/components/review/CommentItem.tsx` | hiển thị range |
| Frontend | `frontend/src/components/review/LineCommentPopover.tsx` | position.lineNumberEnd |
| Frontend | `frontend/src/components/diff-viewer/DiffViewer.tsx` | onAddComment truyền line_number_end |
| Frontend | `frontend/src/components/diff-viewer/DiffLine.tsx` | isSelected, isRangeStart, isRangeEnd (optional) |

---

## 8. API Documentation

Cập nhật `docs/api/15-reviews.md`:

```yaml
# POST /api/v1/attempts/{id}/comments
request_body:
  content: string (required)
  file_path: string (optional)
  line_number: number (optional)  # Start line for line/range comment
  line_number_end: number (optional)  # End line for range. Must be >= line_number. Omit for single-line.

# Response - ReviewComment
  line_number: number | null
  line_number_end: number | null  # NEW
```

---

## 9. Testing

- Unit test: `format_comments_as_feedback` với single-line và multi-line
- Integration test: POST comment với `line_number_end`, verify response
- E2E: click range → add comment → verify hiển thị đúng
