# Code Review Components - Phase 5.6

Interactive code review UI with inline comments (frontend-only with mock API).

## Components

### InlineComment
Display single comment with author/timestamp.
```tsx
<InlineComment comment={comment} onDelete={handleDelete} canDelete={true} />
```

### CommentThread
Thread of comments for a line with collapse support.
```tsx
<CommentThread comments={lineComments} lineNumber={42} onDeleteComment={...} />
```

### CommentInput
Input form with markdown preview and keyboard shortcuts.
```tsx
<CommentInput lineNumber={42} onSubmit={handleSubmit} onCancel={handleCancel} />
```

### ReviewActionBar
Approve/request changes workflow with summary input.
```tsx
<ReviewActionBar onReviewSubmitted={handleSubmit} />
```

## Context

### ReviewProvider + useReview()
Manages review state and actions.
```tsx
<ReviewProvider attemptId={attemptId} currentUserId={userId}>
  <DiffsPanel />
  <ReviewActionBar />
</ReviewProvider>
```

## Integration Example
See `DiffWithComments.example.tsx` for complete pattern.

## Mock API
`src/api/code-review-mock.ts` - Replace with real backend when ready.

## Backend Integration
Replace mocks with:
- `POST /api/v1/attempts/{id}/reviews/comments`
- `DELETE /api/v1/reviews/comments/{commentId}`
- `POST /api/v1/attempts/{id}/reviews/submit`
