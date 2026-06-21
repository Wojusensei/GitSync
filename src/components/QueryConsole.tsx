import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { motion } from 'framer-motion';

interface QueryResult {
  columns: string[];
  rows: string[][];
  elapsed_ms: number;
}

export default function QueryConsole({ repoPath }: { repoPath: string }) {
  const [sql, setSql] = useState('');
  const [result, setResult] = useState<QueryResult | null>(null);
  const [error, setError] = useState('');
  const [running, setRunning] = useState(false);

  const runQuery = async () => {
    if (!sql.trim()) return;
    setRunning(true);
    setError('');
    setResult(null);
    try {
      const res = await invoke<QueryResult>('git_query', { path: repoPath, sql });
      setResult(res);
    } catch (e: any) {
      setError(String(e));
    } finally {
      setRunning(false);
    }
  };

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}>
      <h3>Git SQL 查询</h3>
      <textarea
        className="path-input"
        value={sql}
        onChange={e => setSql(e.target.value)}
        placeholder="输入 SQL 查询...&#10;例如: SELECT * FROM commits WHERE author = '你的名字' LIMIT 10"
        style={{ minHeight: 100, fontFamily: 'monospace' }}
      />
      <div style={{ display: 'flex', gap: 8, marginTop: 8 }}>
        <button className="btn btn-blue" onClick={runQuery} disabled={running}>
          {running ? '查询中...' : '执行'}
        </button>
        {result && (
          <span className="time" style={{ fontSize: 12, color: '#8899aa', alignSelf: 'center' }}>
            耗时: {result.elapsed_ms}ms
          </span>
        )}
      </div>
      {error && <div className="analysis-item" style={{ color: '#ff6b6b' }}>{error}</div>}
      {result && (
        <div style={{ marginTop: 12, overflowX: 'auto' }}>
          <table className="query-table">
            <thead>
              <tr>
                {result.columns.map((col, i) => (
                  <th key={i}>{col}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {result.rows.map((row, i) => (
                <tr key={i}>
                  {row.map((cell, j) => (
                    <td key={j}>{cell}</td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </motion.div>
  );
}