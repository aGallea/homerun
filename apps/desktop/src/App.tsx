import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";

function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<Navigate to="/dashboard" replace />} />
        <Route
          path="/dashboard"
          element={
            <div style={{ padding: 32 }}>
              <h1>HomeRun</h1>
              <p>Desktop app scaffold — UI coming soon.</p>
            </div>
          }
        />
      </Routes>
    </BrowserRouter>
  );
}

export default App;
