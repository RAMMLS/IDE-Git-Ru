import { useEffect, useState } from 'react'

interface RepoInfo {
  current_branch: string;
  commit_count: number;
}

function App() {
  const [info, setInfo] = useState<RepoInfo | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    fetch('http://localhost:3000/api/repo/info')
      .then(res => res.json())
      .then(data => {
        if (data.error) {
          setError(data.error)
        } else {
          setInfo(data)
        }
      })
      .catch(err => {
        setError('Failed to fetch from backend. Is the server running on port 3000?')
        console.error(err)
      })
  }, [])

  return (
    <div className="min-h-screen bg-gray-50 flex items-center justify-center p-4">
      <div className="max-w-md w-full bg-white rounded-xl shadow-lg p-8">
        <div className="text-center">
          <h1 className="text-3xl font-bold text-gray-900 mb-2">Aura VCS</h1>
          <p className="text-gray-500 mb-8">Repository Dashboard</p>
        </div>

        {error ? (
          <div className="bg-red-50 text-red-600 p-4 rounded-lg border border-red-200">
            {error}
          </div>
        ) : info ? (
          <div className="space-y-4">
            <div className="bg-blue-50 border border-blue-200 p-4 rounded-lg flex justify-between items-center">
              <span className="text-blue-700 font-medium">Current Branch:</span>
              <span className="bg-blue-100 text-blue-800 px-3 py-1 rounded-full text-sm font-semibold">
                {info.current_branch}
              </span>
            </div>
            <div className="bg-green-50 border border-green-200 p-4 rounded-lg flex justify-between items-center">
              <span className="text-green-700 font-medium">Total Commits:</span>
              <span className="bg-green-100 text-green-800 px-3 py-1 rounded-full text-sm font-semibold">
                {info.commit_count}
              </span>
            </div>
          </div>
        ) : (
          <div className="flex justify-center items-center py-8">
            <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-gray-900"></div>
          </div>
        )}
      </div>
    </div>
  )
}

export default App
