import { Link, useNavigate } from 'react-router-dom';
import { logout } from '../../api/auth';

export function Header() {
  const navigate = useNavigate();

  const handleLogout = () => {
    logout();
    navigate('/login');
  };

  return (
    <header className="bg-white shadow">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        <div className="flex justify-between items-center py-4">
          <div className="flex items-center space-x-8">
            <Link to="/projects" className="text-xl font-bold text-gray-900">
              ACPMS
            </Link>
            <nav className="flex space-x-4">
              <Link
                to="/projects"
                className="text-gray-700 hover:text-gray-900 px-3 py-2 rounded-md text-sm font-medium"
              >
                Projects
              </Link>
            </nav>
          </div>
          <button
            onClick={handleLogout}
            className="text-gray-700 hover:text-gray-900 px-3 py-2 rounded-md text-sm font-medium"
          >
            Logout
          </button>
        </div>
      </div>
    </header>
  );
}
