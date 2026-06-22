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
  const [history, setHistory] = useState<string[]>([]);
  const [showHelp, setShowHelp] = useState(false);

  const runQuery = async () => {
    if (!sql.trim()) return;
    setRunning(true);
    setError('');
    setResult(null);
    try {
      const res = await invoke<QueryResult>('git_query', { path: repoPath, sql });
      setResult(res);
      setHistory(prev => {
        const newHistory = [sql, ...prev.filter(h => h !== sql)];
        return newHistory.slice(0, 20);
      });
    } catch (e: any) {
      setError(String(e));
    } finally {
      setRunning(false);
    }
  };

  const exportCSV = () => {
    if (!result) return;
    let csv = result.columns.join(',') + '\n';
    result.rows.forEach(row => {
      csv += row.map(cell => `"${cell.replace(/"/g, '""')}"`).join(',') + '\n';
    });
    navigator.clipboard.writeText(csv).then(() => alert('CSV 已复制到剪贴板'));
  };

  const loadHistoryItem = (item: string) => {
    setSql(item);
  };

  const sampleQueries = [
    "SELECT * FROM commits LIMIT 10",
    "SELECT author, COUNT(*) AS cnt FROM commits GROUP BY author ORDER BY cnt DESC",
    "SELECT * FROM commits WHERE author = 'Wojusensei'",
    "SELECT * FROM commits WHERE message CONTAINS 'fix' LIMIT 5",
    "SELECT commits.author, file_changes.file_path FROM commits JOIN file_changes ON commits.hash = file_changes.commit_hash LIMIT 10",
    "SELECT author, COUNT(*) AS total, SUM(file_changes.additions) AS added FROM commits JOIN file_changes ON commits.hash = file_changes.commit_hash GROUP BY author ORDER BY added DESC LIMIT 20",
  ];

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}>
      <h3 style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        GitSQL 查询
        <button className="btn" onClick={() => setShowHelp(!showHelp)} style={{ fontSize: 12, padding: '2px 10px', background: 'rgba(255,255,255,0.05)', border: '1px solid rgba(255,255,255,0.1)', borderRadius: 6, color: '#8899aa', cursor: 'pointer' }}>
          {showHelp ? '隐藏帮助' : '语法帮助'}
        </button>
      </h3>

      {showHelp && (
        <div style={{ background: 'rgba(255,255,255,0.03)', border: '1px solid rgba(255,255,255,0.06)', borderRadius: 8, padding: 12, marginBottom: 12, fontSize: 12, color: '#8899aa', maxHeight: 200, overflowY: 'auto' }}>
          <div className="section-title" style={{ marginBottom: 8 }}>支持语法</div>
          <div style={{ fontFamily: 'monospace', lineHeight: 1.8 }}>
            <div>SELECT column1, column2, ... FROM table_name</div>
            <div>JOIN table2 ON table1.col = table2.col</div>
            <div>WHERE column = 'value' AND column2 &gt; '2026-01-01'</div>
            <div>WHERE column CONTAINS 'keyword'</div>
            <div>GROUP BY column</div>
            <div>ORDER BY column ASC | DESC</div>
            <div>LIMIT number</div>
            <div style={{ marginTop: 8 }}>聚合函数: COUNT(*), SUM(col), AVG(col), MAX(col), MIN(col)</div>
            <div>别名: SELECT col AS alias_name</div>
            <div style={{ marginTop: 8 }}>可用表: <span style={{ color: '#5B9BD5' }}>commits</span>, <span style={{ color: '#5B9BD5' }}>file_changes</span></div>
          </div>
        </div>
      )}

      <textarea
        className="path-input"
        value={sql}
        onChange={e => setSql(e.target.value)}
        placeholder="输入 SQL 查询...&#10;例如: SELECT * FROM commits LIMIT 10"
        style={{ minHeight: 80, fontFamily: 'monospace' }}
      />

      <div style={{ display: 'flex', gap: 8, marginTop: 8, flexWrap: 'wrap' }}>
        <button className="btn btn-blue" onClick={runQuery} disabled={running}>
          {running ? '查询中...' : '执行'}
        </button>
        {result && (
          <>
            <span className="time" style={{ fontSize: 12, color: '#8899aa', alignSelf: 'center' }}>
              耗时: {result.elapsed_ms}ms | {result.rows.length} 行
            </span>
            <button className="btn" onClick={exportCSV} style={{ fontSize: 12, padding: '4px 12px', background: 'rgba(255,255,255,0.05)', border: '1px solid rgba(255,255,255,0.1)', borderRadius: 6, color: '#8899aa', cursor: 'pointer' }}>
              导出 CSV
            </button>
          </>
        )}
      </div>

      {error && <div className="analysis-item" style={{ color: '#ff6b6b' }}>{error}</div>}

      {sampleQueries.length > 0 && !result && !running && (
        <div style={{ marginTop: 12 }}>
          <div className="section-title" style={{ marginBottom: 6 }}>示例查询</div>
          {sampleQueries.map((q, i) => (
            <div key={i} className="analysis-item" onClick={() => setSql(q)} style={{ cursor: 'pointer', fontFamily: 'monospace', fontSize: 11 }}>
              {q}
            </div>
          ))}
        </div>
      )}

      {history.length > 0 && (
        <div style={{ marginTop: 12 }}>
          <div className="section-title" style={{ marginBottom: 6 }}>查询历史</div>
          {history.map((h, i) => (
            <div key={i} className="analysis-item" onClick={() => loadHistoryItem(h)} style={{ cursor: 'pointer', fontFamily: 'monospace', fontSize: 11 }}>
              {h}
            </div>
          ))}
        </div>
      )}

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