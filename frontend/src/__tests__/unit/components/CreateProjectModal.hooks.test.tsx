import { render } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { CreateProjectModal } from '../../../components/modals/CreateProjectModal';

const mockNavigate = vi.fn();

vi.mock('react-router-dom', async () => {
  const actual = await vi.importActual<typeof import('react-router-dom')>(
    'react-router-dom'
  );
  return {
    ...actual,
    useNavigate: () => mockNavigate,
  };
});

describe('CreateProjectModal hook order', () => {
  it('does not throw when toggling isOpen from false to true', () => {
    const onClose = vi.fn();
    const { rerender } = render(
      <CreateProjectModal isOpen={false} onClose={onClose} />
    );

    expect(() =>
      rerender(<CreateProjectModal isOpen={true} onClose={onClose} />)
    ).not.toThrow();
  });
});
